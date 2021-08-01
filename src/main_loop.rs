#[cfg(feature = "dbus_mpris")]
use crate::dbus_mpris::DbusServer;
use crate::process::{spawn_program_on_event, Child};
use futures::{self, Future, Stream, StreamExt};
use librespot::core::session::SessionError;
use librespot::playback::config::AudioFormat;
use librespot::{
    connect::{discovery::DiscoveryStream, spirc::Spirc},
    core::{
        cache::Cache,
        config::{ConnectConfig, DeviceType, SessionConfig, VolumeCtrl},
        session::Session,
    },
    playback::{
        audio_backend::Sink,
        config::PlayerConfig,
        mixer::Mixer,
        player::{Player, PlayerEvent},
    },
};
use log::error;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio_stream::wrappers::UnboundedReceiverStream;

pub struct LibreSpotConnection {
    connection: Pin<Box<dyn Future<Output = Result<Session, SessionError>>>>,
    spirc_task: Option<Pin<Box<dyn Future<Output = ()>>>>,
    spirc: Option<Arc<Spirc>>,
    discovery_stream: DiscoveryStream,
}

impl LibreSpotConnection {
    pub fn new(
        connection: Pin<Box<dyn Future<Output = Result<Session, SessionError>>>>,
        discovery_stream: DiscoveryStream,
    ) -> LibreSpotConnection {
        LibreSpotConnection {
            connection,
            spirc_task: None,
            spirc: None,
            discovery_stream,
        }
    }
}

pub struct AudioSetup {
    pub mixer: Box<dyn FnMut() -> Box<dyn Mixer>>,
    pub backend: fn(Option<String>, AudioFormat) -> Box<dyn Sink>,
    pub audio_device: Option<String>,
}

pub struct SpotifydState {
    // TODO: this ain't a stream anymore, rename
    pub ctrl_c_stream: Pin<Box<dyn Future<Output = Result<(), io::Error>>>>,
    pub shutting_down: bool,
    pub cache: Option<Cache>,
    pub device_name: String,
    pub player_event_channel: Option<Pin<Box<dyn Stream<Item = PlayerEvent>>>>,
    pub player_event_program: Option<String>,
    pub dbus_mpris_server: Option<Pin<Box<dyn Future<Output = ()>>>>,
}

#[cfg(feature = "dbus_mpris")]
#[allow(clippy::unnecessary_wraps)]
fn new_dbus_server(
    session: Session,
    spirc: Arc<Spirc>,
    device_name: String,
) -> Option<Pin<Box<dyn Future<Output = ()>>>> {
    Some(Box::pin(DbusServer::new(session, spirc, device_name)))
}

#[cfg(not(feature = "dbus_mpris"))]
fn new_dbus_server(
    _: Session,
    _: Arc<Spirc>,
    _: String,
) -> Option<Pin<Box<dyn Future<Output = ()>>>> {
    None
}

pub(crate) struct MainLoopState {
    pub(crate) librespot_connection: LibreSpotConnection,
    pub(crate) audio_setup: AudioSetup,
    pub(crate) spotifyd_state: SpotifydState,
    pub(crate) player_config: PlayerConfig,
    pub(crate) session_config: SessionConfig,
    pub(crate) autoplay: bool,
    pub(crate) volume_ctrl: VolumeCtrl,
    pub(crate) initial_volume: Option<u16>,
    pub(crate) running_event_program: Option<Child>,
    pub(crate) shell: String,
    pub(crate) device_type: DeviceType,
    pub(crate) use_mpris: bool,
}

impl Future for MainLoopState {
    type Output = ();

    fn poll(mut self: Pin<&mut MainLoopState>, cx: &mut Context<'_>) -> Poll<()> {
        loop {
            if let Poll::Ready(Some(creds)) = self
                .as_mut()
                .librespot_connection
                .discovery_stream
                .poll_next_unpin(cx)
            {
                if let Some(ref mut spirc) = self.librespot_connection.spirc {
                    spirc.shutdown();
                }
                let session_config = self.session_config.clone();
                let cache = self.spotifyd_state.cache.clone();
                // TODO: a bunch of this init logic can probably be unrolled using async / await
                self.librespot_connection.connection =
                    Box::pin(Session::connect(session_config, creds, cache));
            }

            if let Some(mut child) = self.running_event_program.take() {
                match child.try_wait() {
                    // Still running...
                    Ok(None) => self.running_event_program = Some(child),
                    // Exited with error...
                    Err(e) => error!("{}", e),
                    // Exited without error...
                    Ok(Some(_)) => (),
                }
            }
            if self.running_event_program.is_none() {
                if let Some(ref mut player_event_channel) = self.spotifyd_state.player_event_channel
                {
                    if let Poll::Ready(Some(event)) = player_event_channel.poll_next_unpin(cx) {
                        if let Some(ref cmd) = self.spotifyd_state.player_event_program {
                            match spawn_program_on_event(&self.shell, cmd, event) {
                                Ok(child) => self.running_event_program = Some(child),
                                Err(e) => error!("{}", e),
                            }
                        }
                    }
                }
            }

            if let Some(ref mut fut) = self.spotifyd_state.dbus_mpris_server {
                let _ = fut.as_mut().poll(cx);
            }

            if let Poll::Ready(Ok(session)) = self.librespot_connection.connection.as_mut().poll(cx)
            {
                let mixer = (self.audio_setup.mixer)();
                let audio_filter = mixer.get_audio_filter();
                self.librespot_connection.connection = Box::pin(futures::future::pending());
                let backend = self.audio_setup.backend;
                let audio_device = self.audio_setup.audio_device.clone();
                let (player, event_channel) = Player::new(
                    self.player_config.clone(),
                    session.clone(),
                    audio_filter,
                    // TODO: dunno how to work with AudioFormat yet, maybe dig further if this
                    // doesn't work for all configurations
                    move || (backend)(audio_device, AudioFormat::default()),
                );

                self.spotifyd_state.player_event_channel =
                    Some(Box::pin(UnboundedReceiverStream::new(event_channel)));

                let (spirc, spirc_task) = Spirc::new(
                    ConnectConfig {
                        autoplay: self.autoplay,
                        name: self.spotifyd_state.device_name.clone(),
                        device_type: self.device_type,
                        volume: self.initial_volume.unwrap_or_else(|| mixer.volume()),
                        volume_ctrl: self.volume_ctrl.clone(),
                    },
                    session.clone(),
                    player,
                    mixer,
                );
                self.librespot_connection.spirc_task = Some(Box::pin(spirc_task));
                let shared_spirc = Arc::new(spirc);
                self.librespot_connection.spirc = Some(shared_spirc.clone());

                if self.use_mpris {
                    self.spotifyd_state.dbus_mpris_server = new_dbus_server(
                        session,
                        shared_spirc,
                        self.spotifyd_state.device_name.clone(),
                    );
                }
            } else if let Poll::Ready(_) = self.spotifyd_state.ctrl_c_stream.as_mut().poll(cx) {
                if !self.spotifyd_state.shutting_down {
                    if let Some(ref spirc) = self.librespot_connection.spirc {
                        spirc.shutdown();
                        self.spotifyd_state.shutting_down = true;
                    }
                    return Poll::Ready(());
                }
            } else if let Some(Poll::Ready(_)) = self
                .librespot_connection
                .spirc_task
                .as_mut()
                .map(|ref mut st| st.as_mut().poll(cx))
            {
                return Poll::Ready(());
            } else {
                return Poll::Pending;
            }
        }
    }
}

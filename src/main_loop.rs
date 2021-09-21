#[cfg(feature = "dbus_mpris")]
use crate::dbus_mpris::DbusServer;
use crate::process::{spawn_program, Child};
use futures::{self, Async, Future, Poll, Stream};
use librespot::{
    connect::{
        discovery::DiscoveryStream,
        spirc::{Spirc, SpircTask},
    },
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
use std::collections::HashMap;
use std::{io, rc::Rc};
use tokio_core::reactor::Handle;
use tokio_io::IoStream;

pub struct LibreSpotConnection {
    connection: Box<dyn Future<Item = Session, Error = io::Error>>,
    spirc_task: Option<SpircTask>,
    spirc: Option<Rc<Spirc>>,
    discovery_stream: DiscoveryStream,
}

impl LibreSpotConnection {
    pub fn new(
        connection: Box<dyn Future<Item = Session, Error = io::Error>>,
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
    pub backend: fn(Option<String>) -> Box<dyn Sink>,
    pub audio_device: Option<String>,
}

pub struct SpotifydState {
    pub ctrl_c_stream: IoStream<()>,
    pub shutting_down: bool,
    pub cache: Option<Cache>,
    pub device_name: String,
    pub player_event_channel: Option<futures::sync::mpsc::UnboundedReceiver<PlayerEvent>>,
    pub player_event_track_change_program: Option<String>,
    pub player_event_track_started_program: Option<String>,
    pub player_event_track_stopped_program: Option<String>,
    pub player_event_track_loading_program: Option<String>,
    pub player_event_track_playing_program: Option<String>,
    pub player_event_track_paused_program: Option<String>,
    pub player_event_track_preload_program: Option<String>,
    pub player_event_track_end_program: Option<String>,
    pub player_event_volume_set_program: Option<String>,
    pub player_event_track_unavailable_program: Option<String>,
    pub dbus_mpris_server: Option<Box<dyn Future<Item = (), Error = ()>>>,
}

#[cfg(feature = "dbus_mpris")]
#[allow(clippy::unnecessary_wraps)]
fn new_dbus_server(
    session: Session,
    handle: Handle,
    spirc: Rc<Spirc>,
    device_name: String,
) -> Option<Box<dyn Future<Item = (), Error = ()>>> {
    Some(Box::new(DbusServer::new(
        session,
        handle,
        spirc,
        device_name,
    )))
}

#[cfg(not(feature = "dbus_mpris"))]
fn new_dbus_server(
    _: Session,
    _: Handle,
    _: Rc<Spirc>,
    _: String,
) -> Option<Box<dyn Future<Item = (), Error = ()>>> {
    None
}

pub(crate) struct MainLoopState {
    pub(crate) librespot_connection: LibreSpotConnection,
    pub(crate) audio_setup: AudioSetup,
    pub(crate) spotifyd_state: SpotifydState,
    pub(crate) player_config: PlayerConfig,
    pub(crate) session_config: SessionConfig,
    pub(crate) handle: Handle,
    pub(crate) autoplay: bool,
    pub(crate) volume_ctrl: VolumeCtrl,
    pub(crate) initial_volume: Option<u16>,
    pub(crate) running_event_program: Option<Child>,
    pub(crate) shell: String,
    pub(crate) device_type: DeviceType,
    pub(crate) use_mpris: bool,
}

impl Future for MainLoopState {
    type Error = ();
    type Item = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            if let Async::Ready(Some(creds)) =
                self.librespot_connection.discovery_stream.poll().unwrap()
            {
                if let Some(ref mut spirc) = self.librespot_connection.spirc {
                    spirc.shutdown();
                }
                let session_config = self.session_config.clone();
                let cache = self.spotifyd_state.cache.clone();
                let handle = self.handle.clone();
                self.librespot_connection.connection =
                    Session::connect(session_config, creds, cache, handle);
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
                    if let Async::Ready(Some(event)) = player_event_channel.poll().unwrap() {
                        let mut env = HashMap::new();
                        match event {
                            PlayerEvent::Changed {
                                old_track_id,
                                new_track_id,
                            } => {
                                env.insert("OLD_TRACK_ID", old_track_id.to_base62());
                                env.insert("PLAYER_EVENT", "change".to_string());
                                env.insert("TRACK_ID", new_track_id.to_base62());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_change_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::Started {
                                track_id,
                                play_request_id,
                                position_ms,
                            } => {
                                env.insert("PLAYER_EVENT", "start".to_string());
                                env.insert("TRACK_ID", track_id.to_base62());
                                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                                env.insert("POSITION_MS", position_ms.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_started_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::Stopped {
                                track_id,
                                play_request_id,
                            } => {
                                env.insert("PLAYER_EVENT", "start".to_string());
                                env.insert("TRACK_ID", track_id.to_base62());
                                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_stopped_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::Loading {
                                track_id,
                                play_request_id,
                                position_ms,
                            } => {
                                env.insert("PLAYER_EVENT", "start".to_string());
                                env.insert("TRACK_ID", track_id.to_base62());
                                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                                env.insert("POSITION_MS", position_ms.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_loading_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::Playing {
                                track_id,
                                play_request_id,
                                position_ms,
                                duration_ms,
                            } => {
                                env.insert("PLAYER_EVENT", "play".to_string());
                                env.insert("TRACK_ID", track_id.to_base62());
                                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                                env.insert("POSITION_MS", position_ms.to_string());
                                env.insert("DURATION_MS", duration_ms.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_playing_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::Paused {
                                track_id,
                                play_request_id,
                                position_ms,
                                duration_ms,
                            } => {
                                env.insert("PLAYER_EVENT", "pause".to_string());
                                env.insert("TRACK_ID", track_id.to_base62());
                                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                                env.insert("POSITION_MS", position_ms.to_string());
                                env.insert("DURATION_MS", duration_ms.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_paused_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::TimeToPreloadNextTrack {
                                track_id,
                                play_request_id,
                            } => {
                                env.insert("PLAYER_EVENT", "preload".to_string());
                                env.insert("TRACK_ID", track_id.to_base62());
                                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_preload_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::EndOfTrack {
                                track_id,
                                play_request_id,
                            } => {
                                env.insert("PLAYER_EVENT", "endoftrack".to_string());
                                env.insert("TRACK_ID", track_id.to_base62());
                                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_end_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::VolumeSet { volume } => {
                                env.insert("PLAYER_EVENT", "volumeset".to_string());
                                env.insert("VOLUME", volume.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_volume_set_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                            PlayerEvent::Unavailable {
                                play_request_id,
                                track_id,
                            } => {
                                env.insert("PLAYER_EVENT", "unavailable".to_string());
                                env.insert("TRACK_ID", track_id.to_base62());
                                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                                if let Some(ref cmd) =
                                    self.spotifyd_state.player_event_track_unavailable_program
                                {
                                    match spawn_program(&self.shell, cmd, env) {
                                        Ok(child) => self.running_event_program = Some(child),
                                        Err(e) => error!("{}", e),
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(ref mut fut) = self.spotifyd_state.dbus_mpris_server {
                let _ = fut.poll();
            }

            if let Async::Ready(session) = self.librespot_connection.connection.poll().unwrap() {
                let mixer = (self.audio_setup.mixer)();
                let audio_filter = mixer.get_audio_filter();
                self.librespot_connection.connection = Box::new(futures::future::empty());
                let backend = self.audio_setup.backend;
                let audio_device = self.audio_setup.audio_device.clone();
                let (player, event_channel) = Player::new(
                    self.player_config.clone(),
                    session.clone(),
                    audio_filter,
                    move || (backend)(audio_device),
                );

                self.spotifyd_state.player_event_channel = Some(event_channel);

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
                self.librespot_connection.spirc_task = Some(spirc_task);
                let shared_spirc = Rc::new(spirc);
                self.librespot_connection.spirc = Some(shared_spirc.clone());

                if self.use_mpris {
                    self.spotifyd_state.dbus_mpris_server = new_dbus_server(
                        session,
                        self.handle.clone(),
                        shared_spirc,
                        self.spotifyd_state.device_name.clone(),
                    );
                }
            } else if let Async::Ready(_) = self.spotifyd_state.ctrl_c_stream.poll().unwrap() {
                if !self.spotifyd_state.shutting_down {
                    if let Some(ref spirc) = self.librespot_connection.spirc {
                        spirc.shutdown();
                        self.spotifyd_state.shutting_down = true;
                    } else {
                        return Ok(Async::Ready(()));
                    }
                }
            } else if let Some(Async::Ready(_)) = self
                .librespot_connection
                .spirc_task
                .as_mut()
                .map(|ref mut st| st.poll().unwrap())
            {
                return Ok(Async::Ready(()));
            } else {
                return Ok(Async::NotReady);
            }
        }
    }
}

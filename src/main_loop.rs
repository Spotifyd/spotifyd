#[cfg(feature = "dbus_mpris")]
use crate::config::{DBusType, MprisConfig};
#[cfg(feature = "dbus_mpris")]
use crate::dbus_mpris::DbusServer;
use crate::process::spawn_program_on_event;
use futures::future::Either;
#[cfg(not(feature = "dbus_mpris"))]
use futures::future::Pending;
use futures::{
    self,
    future::{self, Fuse, FusedFuture},
    stream::Peekable,
    Future, FutureExt, StreamExt,
};
use librespot_connect::{config::ConnectConfig, spirc::Spirc};
use librespot_core::{
    authentication::Credentials, cache::Cache, config::DeviceType, session::Session, Error,
    SessionConfig,
};
use librespot_discovery::Discovery;
use librespot_playback::{
    audio_backend::Sink,
    config::{AudioFormat, PlayerConfig},
    mixer::Mixer,
    player::Player,
};
use log::error;
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(feature = "dbus_mpris"))]
type DbusServer = Pending<()>;

pub(crate) enum CredentialsProvider {
    Discovery(Peekable<Discovery>),
    SpotifyCredentials(Credentials),
}

impl From<Discovery> for CredentialsProvider {
    fn from(stream: Discovery) -> Self {
        CredentialsProvider::Discovery(stream.peekable())
    }
}

impl CredentialsProvider {
    async fn get_credentials(&mut self) -> Credentials {
        match self {
            CredentialsProvider::Discovery(stream) => stream.next().await.unwrap(),
            CredentialsProvider::SpotifyCredentials(creds) => creds.clone(),
        }
    }

    // wait for an incoming connection if the underlying provider is a discovery stream
    async fn incoming_connection(&mut self) {
        match self {
            CredentialsProvider::Discovery(stream) => {
                let peeked = Pin::new(stream).peek().await;
                if peeked.is_none() {
                    future::pending().await
                }
            }
            _ => future::pending().await,
        }
    }
}

pub(crate) struct MainLoop {
    pub(crate) session_config: SessionConfig,
    pub(crate) player_config: PlayerConfig,
    pub(crate) cache: Option<Cache>,
    pub(crate) mixer: Arc<dyn Mixer>,
    pub(crate) backend: fn(Option<String>, AudioFormat) -> Box<dyn Sink>,
    pub(crate) audio_device: Option<String>,
    pub(crate) audio_format: AudioFormat,
    pub(crate) has_volume_ctrl: bool,
    pub(crate) initial_volume: Option<u16>,
    pub(crate) shell: String,
    pub(crate) device_type: DeviceType,
    pub(crate) device_name: String,
    pub(crate) player_event_program: Option<String>,
    pub(crate) credentials_provider: CredentialsProvider,
    #[cfg(feature = "dbus_mpris")]
    pub(crate) mpris_config: MprisConfig,
}

struct ConnectionInfo<SpircTask: Future<Output = ()>> {
    spirc: Spirc,
    #[cfg_attr(not(feature = "dbus_mpris"), expect(unused))]
    session: Session,
    player: Arc<Player>,
    spirc_task: SpircTask,
}

impl MainLoop {
    async fn get_connection(&mut self) -> Result<ConnectionInfo<impl Future<Output = ()>>, Error> {
        let creds = self.credentials_provider.get_credentials().await;

        let session = Session::new(self.session_config.clone(), self.cache.clone());
        let player = {
            let audio_device = self.audio_device.clone();
            let audio_format = self.audio_format;
            let backend = self.backend;
            Player::new(
                self.player_config.clone(),
                session.clone(),
                self.mixer.get_soft_volume(),
                move || backend(audio_device, audio_format),
            )
        };

        // TODO: expose is_group
        Spirc::new(
            ConnectConfig {
                name: self.device_name.clone(),
                device_type: self.device_type,
                is_group: false,
                initial_volume: self.initial_volume,
                has_volume_ctrl: self.has_volume_ctrl,
            },
            session.clone(),
            creds,
            player.clone(),
            self.mixer.clone(),
        )
        .await
        .map(|(spirc, spirc_task)| ConnectionInfo {
            spirc,
            session,
            player,
            spirc_task,
        })
    }

    pub(crate) async fn run(mut self) {
        tokio::pin! {
            let ctrl_c = tokio::signal::ctrl_c();
            // we don't necessarily have a dbus server
            let dbus_server = Either::<DbusServer, _>::Right(future::pending());
        }

        #[cfg(feature = "dbus_mpris")]
        let mpris_event_tx = if self.mpris_config.use_mpris.unwrap_or(true) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            *dbus_server.as_mut() = Either::Left(DbusServer::new(
                rx,
                self.mpris_config.dbus_type.unwrap_or(DBusType::Session),
            ));
            Some(tx)
        } else {
            None
        };

        'mainloop: loop {
            let connection = tokio::select!(
                _ = &mut ctrl_c => {
                    if let CredentialsProvider::Discovery(stream) = self.credentials_provider {
                        let _ = stream.into_inner().shutdown().await;
                    }
                    break 'mainloop;
                }
                connection = self.get_connection() => {
                    match connection {
                        Ok(connection) => connection,
                        Err(err) => {
                            error!("failed to connect to spotify: {}", err);
                            break 'mainloop;
                        }
                    }
                }
            );

            let spirc_task = connection.spirc_task;
            tokio::pin!(spirc_task);

            let shared_spirc = Arc::new(connection.spirc);

            #[cfg(feature = "dbus_mpris")]
            if let Either::Left(mut dbus_server) = Either::as_pin_mut(dbus_server.as_mut()) {
                if let Err(err) = dbus_server
                    .as_mut()
                    .set_session(shared_spirc.clone(), connection.session)
                {
                    error!("failed to configure dbus server: {err}");
                    let _ = shared_spirc.shutdown();
                    break 'mainloop;
                }
            }

            let mut running_event_program = Box::pin(Fuse::terminated());

            let mut event_channel = connection.player.get_player_event_channel();

            loop {
                tokio::select!(
                    // a new session has been started via the discovery stream
                    _ = self.credentials_provider.incoming_connection() => {
                        let _ = shared_spirc.shutdown();
                        break;
                    }
                    // the program should shut down
                    _ = &mut ctrl_c => {
                        let _ = shared_spirc.shutdown();
                        break 'mainloop;
                    }
                    // spirc was shut down by some external factor
                    _ = &mut spirc_task => {
                        break;
                    }
                    // dbus stopped unexpectedly
                    result = &mut dbus_server => {
                        #[cfg(feature = "dbus_mpris")]
                        {
                            if let Err(err) = result {
                                error!("DBus terminated unexpectedly: {err}");
                            }
                            let _ = shared_spirc.shutdown();
                            *dbus_server.as_mut() = Either::Right(future::pending());
                            break 'mainloop;
                        }
                        #[cfg(not(feature = "dbus_mpris"))]
                        result // unused variable
                    }
                    // a new player event is available and no program is running
                    event = event_channel.recv(), if running_event_program.is_terminated() => {
                        let event = event.unwrap();
                        #[cfg(feature = "dbus_mpris")]
                        if let Some(ref tx) = mpris_event_tx {
                            tx.send(event.clone()).unwrap();
                        }
                        if let Some(ref cmd) = self.player_event_program {
                            match spawn_program_on_event(&self.shell, cmd, event) {
                                Ok(child) => running_event_program = Box::pin(child.wait().fuse()),
                                Err(e) => error!("{}", e),
                            }
                        }
                    }
                    // a running program has finished
                    result = &mut running_event_program, if !running_event_program.is_terminated() => {
                        match result {
                            // Exited without error...
                            Ok(_) => (),
                            // Exited with error...
                            Err(e) => error!("{}", e),
                        }
                    }
                )
            }
            #[cfg(feature = "dbus_mpris")]
            if let Either::Left(dbus_server) = Either::as_pin_mut(dbus_server.as_mut()) {
                if let Err(err) = dbus_server.drop_session() {
                    error!("failed to reconfigure dbus server: {err}");
                    break 'mainloop;
                }
            }
        }
        #[cfg(feature = "dbus_mpris")]
        if let Either::Left(dbus_server) = Either::as_pin_mut(dbus_server.as_mut()) {
            if dbus_server.shutdown() {
                if let Err(err) = dbus_server.await {
                    error!("failed to shutdown the dbus server: {err}");
                }
            }
        }
    }
}

use futures::{Async, Future, Poll, Stream};
use futures;
use std::io;

use librespot::connect::spirc::{Spirc, SpircTask};
use librespot::core::session::Session;
use librespot::core::config::SessionConfig;
use librespot::playback::player::Player;
use librespot::playback::audio_backend::Sink;
use librespot::connect::discovery::DiscoveryStream;
use librespot::playback::mixer::Mixer;
use librespot::playback::config::PlayerConfig;
use librespot::core::cache::Cache;
use librespot::core::config::{ConnectConfig, DeviceType};

use tokio_core::reactor::Handle;
use tokio_io::IoStream;

pub struct LibreSpotConnection {
    connection: Box<Future<Item = Session, Error = io::Error>>,
    spirc_task: Option<SpircTask>,
    spirc: Option<Spirc>,
    discovery_stream: DiscoveryStream,
}

impl LibreSpotConnection {
    pub fn new(connection: Box<Future<Item = Session, Error = io::Error>>,
               discovery_stream: DiscoveryStream) -> LibreSpotConnection {
        LibreSpotConnection { connection: connection, spirc_task: None, 
            spirc: None, discovery_stream: discovery_stream }
    }
}

pub struct AudioSetup {
    pub mixer: Box<FnMut() -> Box<Mixer>>,
    pub backend: fn(Option<String>) -> Box<Sink>,
    pub audio_device: Option<String>,
}

pub struct SpotifydState {
    pub ctrl_c_stream: IoStream<()>,
    pub shutting_down: bool,
    pub cache: Option<Cache>,
    pub device_name: String,
}


pub struct MainLoopState {
    pub librespot_connection: LibreSpotConnection,
    pub audio_setup: AudioSetup,
    pub spotifyd_state: SpotifydState,
    pub player_config: PlayerConfig,
    pub session_config: SessionConfig,
    pub handle: Handle,
}

impl Future for MainLoopState {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            if let Async::Ready(Some(creds)) = self.librespot_connection.discovery_stream.poll().unwrap() {
                if let Some(ref mut spirc) = self.librespot_connection.spirc {
                    spirc.shutdown();
                }
                let session_config = self.session_config.clone();
                let cache = self.spotifyd_state.cache.clone();
                let handle = self.handle.clone();
                self.librespot_connection.connection = Session::connect(session_config, creds, cache, handle);
            }

            if let Async::Ready(session) = self.librespot_connection.connection.poll().unwrap() {
                let mixer = (self.audio_setup.mixer)();
                let audio_filter = mixer.get_audio_filter();
                self.librespot_connection.connection = Box::new(futures::future::empty());
                let backend = self.audio_setup.backend;
                let audio_device = self.audio_setup.audio_device.clone();
                let player = Player::new(
                    self.player_config.clone(),
                    session.clone(),
                    audio_filter,
                    move || (backend)(audio_device),
                );

                let (spirc, spirc_task) = Spirc::new(
                    ConnectConfig {
                        name: self.spotifyd_state.device_name.clone(),
                        device_type: DeviceType::default(),
                        volume: i32::from(mixer.volume()),
                    },
                    session,
                    player,
                    mixer,
                );
                self.librespot_connection.spirc_task = Some(spirc_task);
                self.librespot_connection.spirc = Some(spirc);
            } else if let Async::Ready(_) = self.spotifyd_state.ctrl_c_stream.poll().unwrap() {
                if !self.spotifyd_state.shutting_down {
                    if let Some(ref spirc) = self.librespot_connection.spirc {
                        spirc.shutdown();
                        self.spotifyd_state.shutting_down = true;
                    } else {
                        return Ok(Async::Ready(()));
                    }
                }
            } else if let Some(Async::Ready(_)) = self.librespot_connection.spirc_task
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

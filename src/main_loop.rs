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

pub struct MainLoopState {
    connection: Box<Future<Item = Session, Error = io::Error>>,
    mixer: Box<FnMut() -> Box<Mixer>>,
    backend: fn(Option<String>) -> Box<Sink>,
    audio_device: Option<String>,
    spirc_task: Option<SpircTask>,
    spirc: Option<Spirc>,
    ctrl_c_stream: IoStream<()>,
    shutting_down: bool,
    cache: Option<Cache>,
    player_config: PlayerConfig,
    session_config: SessionConfig,
    device_name: String,
    handle: Handle,
    discovery_stream: DiscoveryStream,
}

impl MainLoopState {
    pub fn new(
        connection: Box<Future<Item = Session, Error = io::Error>>,
        mixer: Box<FnMut() -> Box<Mixer>>,
        backend: fn(Option<String>) -> Box<Sink>,
        audio_device: Option<String>,
        ctrl_c_stream: IoStream<()>,
        discovery_stream: DiscoveryStream,
        cache: Option<Cache>,
        player_config: PlayerConfig,
        session_config: SessionConfig,
        device_name: String,
        handle: Handle,
    ) -> MainLoopState {
        MainLoopState {
            connection: connection,
            mixer: mixer,
            backend: backend,
            audio_device: audio_device,
            spirc_task: None,
            spirc: None,
            ctrl_c_stream: ctrl_c_stream,
            shutting_down: false,
            cache: cache,
            player_config: player_config,
            session_config: session_config,
            device_name: device_name,
            handle: handle,
            discovery_stream: discovery_stream,
        }
    }
}

impl Future for MainLoopState {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            if let Async::Ready(Some(creds)) = self.discovery_stream.poll().unwrap() {
                if let Some(ref mut spirc) = self.spirc {
                    spirc.shutdown();
                }
                let session_config = self.session_config.clone();
                let cache = self.cache.clone();
                let handle = self.handle.clone();
                self.connection = Session::connect(session_config, creds, cache, handle);
            }

            if let Async::Ready(session) = self.connection.poll().unwrap() {
                let mixer = (self.mixer)();
                let audio_filter = mixer.get_audio_filter();
                self.connection = Box::new(futures::future::empty());
                let backend = self.backend;
                let audio_device = self.audio_device.clone();
                let player = Player::new(
                    self.player_config.clone(),
                    session.clone(),
                    audio_filter,
                    move || (backend)(audio_device),
                );

                let (spirc, spirc_task) = Spirc::new(
                    ConnectConfig {
                        name: self.device_name.clone(),
                        device_type: DeviceType::default(),
                        volume: mixer.volume() as i32,
                    },
                    session,
                    player,
                    mixer,
                );
                self.spirc_task = Some(spirc_task);
                self.spirc = Some(spirc);
            } else if let Async::Ready(_) = self.ctrl_c_stream.poll().unwrap() {
                if !self.shutting_down {
                    if let Some(ref spirc) = self.spirc {
                        spirc.shutdown();
                        self.shutting_down = true;
                    } else {
                        return Ok(Async::Ready(()));
                    }
                }
            } else if let Some(Async::Ready(_)) = self.spirc_task
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

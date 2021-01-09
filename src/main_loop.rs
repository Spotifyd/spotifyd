#[cfg(feature = "dbus_mpris")]
use crate::dbus_mpris::DbusServer;
use crate::process::{spawn_program_on_event, Child};
use futures::{self, Async, Future, Poll, Stream};
use librespot::{
    connect::{
        discovery::DiscoveryStream,
        spirc::{Spirc, SpircTask},
    },
    core::{
        cache::Cache,
        config::{ConnectConfig, DeviceType, SessionConfig},
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
use std::{io, rc::Rc};
use tokio_core::reactor::Handle;
use tokio_io::IoStream;

#[cfg(feature = "dbus_mpris")]
use crate::dbus_mpris::{ChangedTrack, DbusServer, TrackChange};

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
    pub player_event_program: Option<String>,
    pub dbus_mpris_server: Option<Box<dyn Future<Item = (), Error = ()>>>,
}

#[cfg(feature = "dbus_mpris")]
fn new_dbus_server(
    session: Session,
    handle: Handle,
    spirc: Rc<Spirc>,
    device_name: String,
    rx: futures::sync::mpsc::UnboundedReceiver<TrackChange>,
) -> Option<Box<dyn Future<Item = (), Error = ()>>> {
    Some(Box::new(DbusServer::new(
        session,
        handle,
        spirc,
        device_name,
        rx,
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
    pub(crate) linear_volume: bool,
    pub(crate) initial_volume: Option<u16>,
    pub(crate) running_event_program: Option<Child>,
    pub(crate) shell: String,
    pub(crate) device_type: DeviceType,
    pub(crate) session: Option<Session>,
    #[cfg(feature = "dbus_mpris")]
    pub(crate) future_get_metadata: Option<Box<dyn Future<Item = (), Error = ()>>>,
    #[cfg(feature = "dbus_mpris")]
    pub(crate) tx: Option<futures::sync::mpsc::UnboundedSender<TrackChange>>,
}

impl Future for MainLoopState {
    type Error = ();
    type Item = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            #[cfg(feature = "dbus_mpris")]
            {
                if let Some(future_get_metadata2) = &mut self.future_get_metadata {
                    if let Async::Ready(()) = future_get_metadata2.poll().unwrap() {
                        println!("ready");
                        self.future_get_metadata = None;
                    }
                }
            }

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
                        if let Some(ref cmd) = self.spotifyd_state.player_event_program {
                            match spawn_program_on_event(&self.shell, cmd, event.clone()) {
                                Ok(child) => self.running_event_program = Some(child),
                                Err(e) => error!("{}", e),
                            }
                        }

                        #[cfg(feature = "dbus_mpris")]
                        {
                            use librespot::metadata::{Album, Artist, Metadata, Track};
                            match event {
                                PlayerEvent::Changed { new_track_id, .. } => {
                                    let session_clone1 = self.session.as_ref().unwrap();
                                    let session_clone2 = self.session.as_ref().unwrap().clone();
                                    let session_clone3 = self.session.as_ref().unwrap().clone();
                                    let tx2 = self.tx.as_ref().unwrap().clone();

                                    let f = Track::get(&session_clone1, new_track_id)
                                        .and_then(move |plist_track| {
                                            let artist_id = *plist_track.artists.get(0).unwrap();

                                            Album::get(&session_clone2, plist_track.album)
                                                .and_then(move |r| {
                                                    Artist::get(&session_clone3, artist_id)
                                                        .map(|artist| (artist, r))
                                                })
                                                .map(|(artist, r)| (r, plist_track, artist))
                                        })
                                        .and_then(move |(album, track, artist)| {
                                            println!("song: {}, album: {}", track.name, album.name);

                                            tx2.unbounded_send(TrackChange::Changed(
                                                ChangedTrack {
                                                    track: track.name,
                                                    album: album.name,
                                                    artist: artist.name,
                                                },
                                            ))
                                            .unwrap();
                                            Ok(())
                                        })
                                        .map_err(|e| {
                                            println!("some err: {:?}", e);
                                        });
                                    self.future_get_metadata = Some(Box::new(f));
                                    continue;
                                }
                                PlayerEvent::Started { track_id } => {
                                    println!("PlayerEvent::started zz");

                                    let session = &self.session.as_ref().unwrap();
                                    let tx2 = self.tx.as_ref().unwrap().clone();

                                    let f = Track::get(&session, track_id)
                                        .and_then(move |plist_track| {
                                            println!("track: {} ", plist_track.name);

                                            tx2.unbounded_send(TrackChange::Started).unwrap();

                                            Ok(())
                                        })
                                        .map_err(|e| {
                                            println!("some err: {:?}", e);
                                        });
                                    self.future_get_metadata = Some(Box::new(f));
                                    continue;
                                }
                                PlayerEvent::Stopped { track_id } => {
                                    println!("PlayerEvent::stopped zz");

                                    let session = &self.session.as_ref().unwrap();
                                    let tx2 = self.tx.as_ref().unwrap().clone();

                                    let f = Track::get(&session, track_id)
                                        .and_then(move |track| {
                                            println!("track: {} ", track.name);
                                            tx2.unbounded_send(TrackChange::Stopped).unwrap();

                                            Ok(())
                                        })
                                        .map_err(|e| {
                                            println!("some err: {:?}", e);
                                        });
                                    self.future_get_metadata = Some(Box::new(f));
                                    continue;
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
                self.session = Some(session.clone());
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
                        linear_volume: self.linear_volume,
                    },
                    session.clone(),
                    player,
                    mixer,
                );
                self.librespot_connection.spirc_task = Some(spirc_task);
                let shared_spirc = Rc::new(spirc);
                self.librespot_connection.spirc = Some(shared_spirc.clone());

                #[cfg(feature = "dbus_mpris")]
                {
                    let (tx, rx) = futures::sync::mpsc::unbounded::<TrackChange>();
                    self.tx = Some(tx);

                    self.spotifyd_state.dbus_mpris_server = new_dbus_server(
                        session,
                        self.handle.clone(),
                        shared_spirc,
                        self.spotifyd_state.device_name.clone(),
                        rx,
                    );
                }

                #[cfg(not(feature = "dbus_mpris"))]
                {
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

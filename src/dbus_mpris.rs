extern crate dbus;
extern crate dbus_tokio;
extern crate futures;
extern crate tokio_core;

use std::rc::Rc;
use std::thread;
use std::collections::HashMap;
use dbus::{BusType, Connection, MessageItem, NameFlag};
use dbus::arg::{RefArg, Variant};
use dbus::tree::{Access, MethodErr};
use dbus_tokio::tree::{AFactory, ATree, ATreeServer};
use dbus_tokio::AConnection;
use tokio_core::reactor::Handle;
use librespot::connect::spirc::Spirc;
use librespot::core::keymaster::{get_token, Token as LibrespotToken};
use librespot::core::mercury::MercuryError;
use librespot::core::session::Session;
use chrono::prelude::*;

use rspotify::spotify::oauth2::TokenInfo as RspotifyToken;
use rspotify::spotify::util::datetime_to_timestamp;
use rspotify::spotify::client::Spotify;
use rspotify::spotify::senum::*;
use futures::{Async, Future, Poll, Stream};
use futures::sync::oneshot;

pub struct DbusServer {
    session: Session,
    handle: Handle,
    spirc: Rc<Spirc>,
    api_token: RspotifyToken,
    token_request: Option<Box<Future<Item = LibrespotToken, Error = MercuryError>>>,
    dbus_future: Option<Box<Future<Item = (), Error = ()>>>,
    device_name: String,
}

const CLIENT_ID: &str = "2c1ea588dfbc4a989e2426f8385297c3";
const SCOPE: &str = "user-read-playback-state,user-read-private,user-read-birthdate,\
                     user-read-email,playlist-read-private,user-library-read,\
                     user-library-modify,user-top-read,playlist-read-collaborative,\
                     playlist-modify-public,playlist-modify-private,user-follow-read,\
                     user-follow-modify,user-read-currently-playing,user-modify-playback-state,\
                     user-read-recently-played";

impl DbusServer {
    pub fn new(
        session: Session,
        handle: Handle,
        spirc: Rc<Spirc>,
        device_name: String,
    ) -> DbusServer {
        DbusServer {
            session,
            handle,
            spirc,
            api_token: RspotifyToken::default(),
            token_request: None,
            dbus_future: None,
            device_name,
        }
    }

    fn is_token_expired(&self) -> bool {
        let now: DateTime<Utc> = Utc::now();
        match self.api_token.expires_at {
            Some(expires_at) => now.timestamp() > expires_at - 100,
            None => true,
        }
    }
}

impl Future for DbusServer {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        let mut got_new_token = false;
        if self.is_token_expired() {
            if let Some(ref mut fut) = self.token_request {
                if let Async::Ready(token) = fut.poll().unwrap() {
                    self.api_token = RspotifyToken::default()
                        .access_token(&token.access_token)
                        .expires_in(token.expires_in)
                        .expires_at(datetime_to_timestamp(token.expires_in));
                    self.dbus_future = Some(create_dbus_server(
                        self.handle.clone(),
                        self.api_token.clone(),
                        self.spirc.clone(),
                        self.device_name.clone(),
                    ));
                    got_new_token = true;
                }
            } else {
                self.token_request = Some(get_token(&self.session, CLIENT_ID, SCOPE));
            }
        } else {
            if let Some(ref mut fut) = self.dbus_future {
                return fut.poll();
            }
        }

        if got_new_token {
            self.token_request = None;
        }

        Ok(Async::NotReady)
    }
}

fn create_spotify_api(token: &RspotifyToken) -> Spotify {
    Spotify::default().access_token(&token.access_token).build()
}

fn create_dbus_server(
    handle: Handle,
    api_token: RspotifyToken,
    spirc: Rc<Spirc>,
    device_name: String,
) -> Box<Future<Item = (), Error = ()>> {
    let c = Rc::new(Connection::get_private(BusType::Session).unwrap());

    macro_rules! spotify_api_method {
        ([ $sp:ident, $device:ident $(, $m:ident: $t:ty)*] $f:expr) => {
            {
                let device_name = device_name.clone();
                let token = api_token.clone();
                move |m| {
                    let (p, c) = oneshot::channel();
                    let token = token.clone();
                    let device_name = device_name.clone();
                    $(let $m: Result<$t,_> = m.msg.read1();)*
                    thread::spawn(move || {
                        let $sp = create_spotify_api(&token);
                        let $device = Some(device_name);
                        let _ = $f;
                        let _ = p.send(());
                    });
                    let mret = m.msg.method_return();
                    c.map_err(|e| MethodErr::failed(&e)).map(|_| vec![mret])
                }
            }
        }
    }

    macro_rules! spotify_api_property {
        ([ $sp:ident, $device:ident] $f:expr) => {
            {
                let device_name = device_name.clone();
                let token = api_token.clone();
                move |i, _| {
                    let $sp = create_spotify_api(&token);
                    let $device = Some(device_name.clone());
                    let v = $f;
                    i.append(v);
                    Ok(())
                }
            }
        }
    }

    c.register_name(
        "org.mpris.MediaPlayer2.spotifyd",
        NameFlag::ReplaceExisting as u32,
    ).unwrap();

    let spirc_quit = spirc.clone();
    let spirc_play_pause = spirc.clone();

    let f = AFactory::new_afn::<()>();
    let tree = f.tree(ATree::new()).add(
        f.object_path("/org/mpris/MediaPlayer2", ())
            .introspectable()
            .add(
                f.interface("org.mpris.MediaPlayer2.Player", ())
                    .add_m(f.amethod(
                        "Next",
                        (),
                        spotify_api_method!([sp, device] sp.next_track(device)),
                    ))
                    .add_m(f.amethod(
                        "Previous",
                        (),
                        spotify_api_method!([sp, device] sp.previous_track(device)),
                    ))
                    .add_m(f.amethod(
                        "Pause",
                        (),
                        spotify_api_method!([sp, device] sp.pause_playback(device)),
                    ))
                    .add_m(f.amethod("PlayPause", (), move |m| {
                        spirc_play_pause.play_pause();
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    }))
                    .add_m(f.amethod(
                        "Play",
                        (),
                        spotify_api_method!([sp, device] sp.start_playback(device, None, None, None)),
                    ))
                    .add_m(f.amethod(
                        "Stop",
                        (),
                        spotify_api_method!([sp, device]{
                            let _ = sp.seek_track(0, device.clone());
                            let _ = sp.pause_playback(device);
                        }),
                    ))
                    .add_m(f.amethod(
                        "Seek",
                        (),
                        spotify_api_method!([sp, device, pos: u32]{
                            match pos {
                                Ok(p) => { 
                                    if let Ok(Some(playing)) = sp.current_user_playing_track() {
                                        let _ = sp.seek_track(playing.progress_ms.unwrap_or(0) + p, device); 
                                    }
                                },
                                _ => (),
                            };
                        }),
                    ))
                    .add_m(f.amethod(
                        "SetPosition",
                        (),
                        spotify_api_method!([sp, device, pos: u32]
                            match pos {
                                Ok(p) => { let _ = sp.seek_track(p, device); },
                                _ => (),
                            }),
                    ))
                    .add_m(f.amethod(
                        "OpenUri",
                        (),
                        spotify_api_method!([sp, device, uri: String] match uri {
                            Ok(uri) => { let _ = sp.start_playback(device, None, Some(vec![uri]), None); },
                            _ => ()
                        }),
                    ))
                    .add_p(
                        f.property::<String, _>("PlayBackStatus", ())
                            .access(Access::Read)
                            .on_get(spotify_api_property!([sp, _device] 
                              if let Ok(Some(track)) = sp.current_user_playing_track() {
                                  if track.is_playing {
                                      "Playing"
                                  } else {
                                      "Paused"
                                  }
                              } else {
                                  "Stopped"
                              }.to_string())),
                    )
                    .add_p(
                        f.property::<f64, _>("Rate", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(1.0);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<f64, _>("MaximumRate", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(1.0);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<f64, _>("MinimumRate", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(1.0);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<String, _>("LoopStatus", ())
                            .access(Access::Read)
                            .on_get(spotify_api_property!([sp, _device] 
                                if let Ok(Some(player)) = sp.current_playback(None) {
                                    match player.repeat_state {
                                        RepeatState::Off => "None",
                                        RepeatState::Track => "Track",
                                        RepeatState::Context => "Playlist",
                                    }
                                } else {
                                    "None"
                                }.to_string()
                            )),
                    )
                    .add_p(
                        f.property::<String, _>("LoopStatus", ())
                            .access(Access::Read)
                            .on_get(spotify_api_property!([sp, _device] 
                                if let Ok(Some(player)) = sp.current_playback(None) {
                                    match player.repeat_state {
                                        RepeatState::Off => "None",
                                        RepeatState::Track => "Track",
                                        RepeatState::Context => "Playlist",
                                    }
                                } else {
                                    "None"
                                }.to_string()
                            )),
                    )
                    .add_p(
                        f.property::<HashMap<String, Variant<Box<RefArg>>>, _>("Metadata", ())
                            .access(Access::Read)
                            .on_get(spotify_api_property!([sp, _device] {
                                let mut m = HashMap::new();
                                let v = sp.current_user_playing_track();
                                if let Ok(Some(playing)) = v {
                                    if let Some(track) = playing.item {
                                        m.insert("xesam:title".to_string(), Variant(Box::new(MessageItem::Str(track.name)) as Box<RefArg>));
                                        m.insert("xesam:album".to_string(), Variant(Box::new(MessageItem::Str(track.album.name)) as Box<RefArg>));
                                        m.insert("xesam:artists".to_string(), 
                                                 Variant(Box::new(track.artists
                                                                  .iter()
                                                                  .map(|a| Box::new(MessageItem::Str(a.name.to_string())) as Box<RefArg>)
                                                                  .collect::<Vec<Box<RefArg>>>())));
                                    }
                                } else {
                                    info!("Couldn't fetch metadata from spotify: {:?}", v);
                                }
                                m
                            })),
                    )
                    .add_p(
                        f.property::<bool, _>("CanPlay", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(true);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<bool, _>("CanPause", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(true);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<bool, _>("CanSeek", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(true);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<bool, _>("CanControl", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(true);
                                Ok(())
                            }),
                    ),
            )
            .add(
                f.interface("org.mpris.MediaPlayer2", ())
                    .add_m(f.amethod("Quit", (), move |m| {
                        spirc_quit.shutdown();
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    }))
                    .add_m(f.amethod("Raise", (), move |m| {
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    }))
                    .add_p(
                        f.property::<bool, _>("CanQuit", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(true);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<bool, _>("CanRaise", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(false);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<bool, _>("HasTrackList", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append(false);
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<String, _>("Identity", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append("Spotifyd".to_string());
                                Ok(())
                            }),
                    )
                    .add_p(
                        f.property::<String, _>("SupportedUriSchemes", ())
                            .access(Access::Read)
                            .on_get(|i, _| {
                                i.append("Spotify".to_string());
                                Ok(())
                            }),
                    ),
            ),
    );

    tree.set_registered(&c, true).unwrap();
    let aconn = AConnection::new(c.clone(), handle).unwrap();
    let server = ATreeServer::new(c.clone(), Box::new(tree), aconn.messages().unwrap());
    Box::new(server.for_each(|m| {
        warn!("Unhandled dbus message: {:?}", m);
        Ok(())
    }))
}

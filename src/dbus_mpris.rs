use chrono::prelude::*;
use dbus::{
    arg::{RefArg, Variant},
    tree::{Access, MethodErr},
    BusType, Connection, MessageItem, MessageItemArray, NameFlag, Signature,
};
use dbus_tokio::{
    tree::{AFactory, ATree, ATreeServer},
    AConnection,
};
use futures::{sync::oneshot, Async, Future, Poll, Stream};
use librespot::{
    connect::spirc::Spirc,
    core::{
        keymaster::{get_token, Token as LibrespotToken},
        mercury::MercuryError,
        session::Session,
    },
};
use log::{info, warn};
use rspotify::spotify::{
    client::Spotify, oauth2::TokenInfo as RspotifyToken, senum::*, util::datetime_to_timestamp,
};
use tokio_core::reactor::Handle;

use std::{collections::HashMap, rc::Rc, thread};

const CLIENT_ID: &str = "2c1ea588dfbc4a989e2426f8385297c3";
const SCOPE: &str = "user-read-playback-state,user-read-private,user-read-birthdate,\
                     user-read-email,playlist-read-private,user-library-read,user-library-modify,\
                     user-top-read,playlist-read-collaborative,playlist-modify-public,\
                     playlist-modify-private,user-follow-read,user-follow-modify,\
                     user-read-currently-playing,user-modify-playback-state,\
                     user-read-recently-played";

pub struct DbusServer {
    session: Session,
    handle: Handle,
    spirc: Rc<Spirc>,
    api_token: RspotifyToken,
    token_request: Option<Box<dyn Future<Item = LibrespotToken, Error = MercuryError>>>,
    dbus_future: Option<Box<dyn Future<Item = (), Error = ()>>>,
    device_name: String,
}

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
    type Error = ();
    type Item = ();

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
        } else if let Some(ref mut fut) = self.dbus_future {
            return fut.poll();
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
) -> Box<dyn Future<Item = (), Error = ()>> {
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
        ([ $sp:ident, $device:ident] $f:expr) => {{
            let device_name = device_name.clone();
            let token = api_token.clone();
            move |i, _| {
                let $sp = create_spotify_api(&token);
                let $device = Some(device_name.clone());
                let v = $f;
                i.append(v);
                Ok(())
            }
        }};
    }

    let connection = Rc::new(
        Connection::get_private(BusType::Session).expect("Failed to initialize DBus connection"),
    );

    connection
        .register_name(
            "org.mpris.MediaPlayer2.spotifyd",
            dbus::NameFlag::ReplaceExisting as u32,
        )
        .expect("Failed to register dbus player name");

    // The tree is asynchronuous so we can fetch data over the spotify web api.
    let f = AFactory::new_afn::<()>();

    // The following properties are part of the MediaPlayer2 interface.
    let property_can_quit = f
        .property::<bool, _>("CanQuit", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(true);
            Ok(())
        });

    let property_can_raise = f
        .property::<bool, _>("CanRaise", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(false);
            Ok(())
        });

    let property_can_fullscreen = f
        .property::<bool, _>("CanSetFullscreen", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(false);
            Ok(())
        });

    let property_has_tracklist = f
        .property::<bool, _>("HasTrackList", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(false);
            Ok(())
        });

    let property_identity = f
        .property::<String, _>("Identity", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append("Spotifyd".to_string());
            Ok(())
        });

    let property_supported_uri_schemes = f
        .property::<Vec<String>, _>("SupportedUriSchemes", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(vec!["spotify".to_string()]);
            Ok(())
        });

    let property_mimetypes = f
        .property::<Vec<String>, _>("SupportedMimeTypes", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(Vec::<String>::new());
            Ok(())
        });

    let media_player2_interface = f
        .interface("org.mpris.MediaPlayer2", ())
        .add_p(property_can_quit)
        .add_p(property_can_raise)
        .add_p(property_can_fullscreen)
        .add_p(property_has_tracklist)
        .add_p(property_identity)
        .add_p(property_supported_uri_schemes)
        .add_p(property_mimetypes);

    // The following methods are part of the MediaPlayer2.Player interface.
    let method_play = {
        let local_spirc = spirc.clone();
        f.method("Play", (), move |m| {
            local_spirc.play();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_pause = {
        let local_spirc = spirc.clone();
        f.method("Pause", (), move |m| {
            local_spirc.pause();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_play_pause = {
        let local_spirc = spirc.clone();
        f.amethod("PlayPause", (), move |m| {
            local_spirc.play_pause();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_stop = {
        let local_spirc = spirc.clone();
        f.amethod("Stop", (), move |m| {
            // TODO: add real stop implementation.
            local_spirc.pause();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_previous = {
        let local_spirc = spirc.clone();
        f.amethod("Previous", (), move |m| {
            local_spirc.prev();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_next = {
        let local_spirc = spirc.clone();
        f.amethod("Next", (), move |m| {
            local_spirc.next();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_seek = {
        let local_spirc = spirc.clone();
        f.amethod(
            "Seek",
            (),
            spotify_api_method!([sp, device, pos: u32]{
                if let Ok(p) = pos {
                    if let Ok(Some(playing)) = sp.current_user_playing_track() {
                        let _ = sp.seek_track(playing.progress_ms.unwrap_or(0) + p, device);
                    }
                }
            }),
        )
    };

    let method_set_position = f.amethod(
            "SetPosition",
            (),
            spotify_api_method!([sp, device, pos: u32]
                if let Ok(p) = pos {
                    let _ = sp.seek_track(p, device);
                }
            ),
        );

    let method_open_uri = {
        f.amethod(
            "OpenUri",
            (),
            spotify_api_method!([sp, device, uri: String]
                if let Ok(uri) = uri {
                    let _ = sp.start_playback(device, None, Some(vec![uri]), None);
                }
            ),
        );
    };

    let media_player2_player_interface = f
        .interface("org.mpris.MediaPlayer2.Player", ())
        .add_m(method_play)
        .add_m(method_pause)
        .add_m(method_play_pause)
        .add_m(method_stop)
        .add_m(method_previous)
        .add_m(method_next);

    let tree = f.tree(ATree::new()).add(
        f.object_path("/org/mpris/MediaPlayer2", ())
            .introspectable()
            .add(media_player2_interface)
            .add(media_player2_player_interface),
    );

    tree.set_registered(&connection, true)
        .expect("Failed to register tree");

    let async_connection = AConnection::new(connection.clone(), handle)
        .expect("Failed to create async dbus connection");
    let server = ATreeServer::new(
        connection.clone(),
        Box::new(tree),
        async_connection
            .messages()
            .expect("Failed to unwrap async messages"),
    );
    Box::new(server.for_each(|message| {
        warn!("Unhandled DBus message: {:?}", message);
        Ok(())
    }))
}

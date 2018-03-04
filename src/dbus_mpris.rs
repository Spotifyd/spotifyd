extern crate dbus;
extern crate dbus_tokio;
extern crate futures;
extern crate tokio_core;

use std::rc::Rc;
use std::thread;
use dbus::{BusType, Connection, NameFlag};
use dbus::tree::MethodErr;
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
const SCOPE: &str = "user-read-private,playlist-read-private,playlist-read-collaborative,\
                     playlist-modify-public,playlist-modify-private,user-follow-modify,\
                     user-follow-read,user-library-read,user-library-modify,user-top-read,\
                     user-read-recently-played,user-modify-playback-state";

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

    macro_rules! spotify_api_call {
        ($f:expr $(, $a:expr)*) => {
            {
                let device_name = device_name.clone();
                let token = api_token.clone();
                move |m| {
                    let (p, c) = oneshot::channel();
                    let token = token.clone();
                    let device_name = device_name.clone();
                    thread::spawn(move || {
                        let sp = create_spotify_api(&token);
                        let _ = $f(&sp, Some(device_name), $($a),*);
                        let _ = p.send(());
                    });
                    let mret = m.msg.method_return();
                    c.map_err(|e| MethodErr::failed(&e)).map(|_| vec![mret])
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
                    .add_m(f.amethod("Next", (), spotify_api_call!(Spotify::next_track)))
                    .add_m(f.amethod("Previous", (), spotify_api_call!(Spotify::previous_track)))
                    .add_m(f.amethod("Pause", (), spotify_api_call!(Spotify::pause_playback)))
                    .add_m(f.amethod("PlayPause", (), move |m| {
                        spirc_play_pause.play_pause();
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    }))
                    .add_m(f.amethod(
                        "Play",
                        (),
                        spotify_api_call!(Spotify::start_playback, None, None, None),
                    )),
            )
            .add(f.interface("org.mpris.MediaPlayer2", ()).add_m(f.amethod(
                "Quit",
                (),
                move |m| {
                    spirc_quit.shutdown();
                    let mret = m.msg.method_return();
                    Ok(vec![mret])
                },
            ))),
    );

    tree.set_registered(&c, true).unwrap();
    let aconn = AConnection::new(c.clone(), handle).unwrap();
    let server = ATreeServer::new(c.clone(), Box::new(tree), aconn.messages().unwrap());
    Box::new(server.for_each(|m| {
        warn!("Unhandled dbus message: {:?}", m);
        Ok(())
    }))
}

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
use futures::{self, Future, FutureExt};
use futures::channel::oneshot;
use librespot::{
    connect::spirc::Spirc,
    core::{
        keymaster::{get_token, Token as LibrespotToken},
        mercury::MercuryError,
        session::Session,
    },
};
use log::{info, warn};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rspotify::spotify::{
    client::Spotify, model::offset::for_position, oauth2::TokenInfo as RspotifyToken, senum::*,
    util::datetime_to_timestamp,
};
use std::{collections::HashMap, env, rc::Rc, thread};
use std::pin::Pin;
use futures::task::{Context, Poll};

pub struct DbusServer {
    session: Session,
    spirc: Rc<Spirc>,
    api_token: RspotifyToken,
    token_request: Option<Pin<Box<dyn Future<Output=Result<LibrespotToken, MercuryError>>>>>,
    dbus_future: Option<Pin<Box<dyn Future<Output = ()>>>>,
    device_name: String,
}

const CLIENT_ID: &str = "2c1ea588dfbc4a989e2426f8385297c3";
const SCOPE: &str = "user-read-playback-state,user-read-private,\
                     user-read-email,playlist-read-private,user-library-read,user-library-modify,\
                     user-top-read,playlist-read-collaborative,playlist-modify-public,\
                     playlist-modify-private,user-follow-read,user-follow-modify,\
                     user-read-currently-playing,user-modify-playback-state,\
                     user-read-recently-played";

impl DbusServer {
    pub fn new(
        session: Session,
        spirc: Rc<Spirc>,
        device_name: String,
    ) -> DbusServer {
        DbusServer {
            session,
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
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut got_new_token = false;
        if self.is_token_expired() {
            if let Some(ref mut fut) = self.token_request {
                if let Poll::Ready(Ok(token)) = fut.as_mut().poll(cx) {
                    self.api_token = RspotifyToken::default()
                        .access_token(&token.access_token)
                        .expires_in(token.expires_in)
                        .expires_at(datetime_to_timestamp(token.expires_in));
                    self.dbus_future = Some(create_dbus_server(
                        self.api_token.clone(),
                        self.spirc.clone(),
                        self.device_name.clone(),
                    ));
                    got_new_token = true;
                }
            } else {
                // This is more meant as a fast hotfix than anything else!
                let client_id =
                    env::var("SPOTIFYD_CLIENT_ID").unwrap_or_else(|_| CLIENT_ID.to_string());
                self.token_request = Some(Box::pin(get_token(&self.session.clone(), &client_id, SCOPE)));
            }
        } else if let Some(ref mut fut) = self.dbus_future {
            return fut.as_mut().poll(cx);
        }

        if got_new_token {
            self.token_request = None;
        }

        Poll::Pending
    }
}

fn create_spotify_api(token: &RspotifyToken) -> Spotify {
    Spotify::default().access_token(&token.access_token).build()
}

fn create_dbus_server(
    api_token: RspotifyToken,
    spirc: Rc<Spirc>,
    device_name: String,
) -> Pin<Box<dyn Future<Output = ()>>> {
    macro_rules! spotify_api_method {
        ([ $sp:ident, $device:ident $(, $m:ident: $t:ty)*] $f:expr) => {
            {
                let device_name = utf8_percent_encode(&device_name, NON_ALPHANUMERIC).to_string();
                let token = api_token.clone();
                move |m| {
                    let (tx, rx) = oneshot::channel();
                    let token = token.clone();
                    let device_name = device_name.clone();
                    $(let $m: Result<$t,_> = m.msg.read1();)*
                    thread::spawn(move || {
                        let $sp = create_spotify_api(&token);
                        let $device = Some(device_name);
                        let _ = $f;
                        let _ = tx.send(());
                    });
                    let mret = m.msg.method_return();
                    rx.map(|val| match val {
                        Ok(_) => Ok(vec![mret]),
                        Err(e) => Err(MethodErr::failed(&e)),
                    }).into()
                }
            }
        }
    }

    macro_rules! spotify_api_property {
        ([ $sp:ident, $device:ident] $f:expr) => {{
            let device_name = utf8_percent_encode(&device_name, NON_ALPHANUMERIC).to_string();
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

    // TODO: allow other DBus types through CLI and config entry.
    let connection = Rc::new(
        Connection::get_private(BusType::Session).expect("Failed to initialize DBus connection"),
    );

    connection
        .register_name(
            "org.mpris.MediaPlayer2.spotifyd",
            NameFlag::ReplaceExisting as u32,
        )
        .expect("Failed to register dbus player name");

    // The tree is asynchronuous so we can fetch data over the spotify web api.
    let f = AFactory::new_afn::<()>();

    // The following methods and properties are part of the MediaPlayer2 interface.
    // https://specifications.freedesktop.org/mpris-spec/latest/Media_Player.html
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

    let method_raise = f.amethod("Raise", (), move |m| {
        let mret = m.msg.method_return();
        Ok(vec![mret])
    });

    let method_quit = {
        let local_spirc = spirc.clone();
        f.amethod("Quit", (), move |m| {
            local_spirc.shutdown();
            let mret = m.msg.method_return();
            Ok(vec![mret])
        })
    };

    let media_player2_interface = f
        .interface("org.mpris.MediaPlayer2", ())
        .add_m(method_raise)
        .add_m(method_quit)
        .add_p(property_can_quit)
        .add_p(property_can_raise)
        .add_p(property_can_fullscreen)
        .add_p(property_has_tracklist)
        .add_p(property_identity)
        .add_p(property_supported_uri_schemes)
        .add_p(property_mimetypes);

    // The following methods and properties are part of the MediaPlayer2.Player interface.
    // https://specifications.freedesktop.org/mpris-spec/latest/Player_Interface.html

    let method_volume_up = {
        let local_spirc = spirc.clone();
        f.amethod("VolumeUp", (), move |m| {
            local_spirc.volume_up();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_volume_down = {
        let local_spirc = spirc.clone();
        f.amethod("VolumeDown", (), move |m| {
            local_spirc.volume_down();
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

    let method_previous = {
        let local_spirc = spirc.clone();
        f.amethod("Previous", (), move |m| {
            local_spirc.prev();
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

    let method_play = {
        let local_spirc = spirc.clone();
        f.method("Play", (), move |m| {
            local_spirc.play();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_stop = {
        let local_spirc = spirc;
        f.amethod("Stop", (), move |m| {
            // TODO: add real stop implementation.
            local_spirc.pause();
            Ok(vec![m.msg.method_return()])
        })
    };

    let method_seek = f.amethod(
        "Seek",
        (),
        spotify_api_method!([sp, device, pos: u32]{
            if let Ok(p) = pos {
                if let Ok(Some(playing)) = sp.current_user_playing_track() {
                    let _ = sp.seek_track(playing.progress_ms.unwrap_or(0) + p, device);
                }
            }
        }),
    );

    let method_set_position = f.amethod(
        "SetPosition",
        (),
        spotify_api_method!([sp, device, pos: u32]
            if let Ok(p) = pos {
                let _ = sp.seek_track(p, device);
            }
        ),
    );

    let method_open_uri = f.amethod(
        "OpenUri",
        (),
        spotify_api_method!([sp, device, uri: String]
            if let Ok(uri) = uri {
                let device_name = device.unwrap_or_else(|| "".to_owned());
                let device_id = match sp.device() {
                    Ok(device_payload) => {
                        match device_payload.devices.into_iter().find(|d| d.is_active && d.name == device_name) {
                            Some(device) => Some(device.id),
                            None => None,
                        }
                    },
                    Err(_) => None,
                };

                if uri.contains("spotify:track") {
                    let _ = sp.start_playback(device_id, None, Some(vec![uri]), for_position(0), None);
                } else {
                    let _ = sp.start_playback(device_id, Some(uri), None, for_position(0), None);
                }
            }
        ),
    );

    let property_playback_status = f
        .property::<String, _>("PlaybackStatus", ())
        .access(Access::Read)
        .on_get(spotify_api_property!([sp, _device]
                    if let Ok(Some(player)) = sp.current_playback(None) {
                        let device_name = utf8_percent_encode(&player.device.name, NON_ALPHANUMERIC).to_string();
                        if device_name == _device.unwrap() {
                            if let Ok(Some(track)) = sp.current_user_playing_track() {
                                if track.is_playing {
                                    "Playing"
                                } else {
                                    "Paused"
                                }
                            } else {
                                "Stopped"
                            }
                        } else {
                            "Stopped"
                        }
                    } else {
                        "Stopped"
                    }.to_string()));

    let property_shuffle = f
        .property::<bool, _>("Shuffle", ())
        .access(Access::Read)
        .on_get(spotify_api_property!([sp, _device]
            if let Ok(Some(player)) = sp.current_playback(None) {
                player.shuffle_state
            } else {
                false
            }
        ));

    let property_rate = f
        .property::<f64, _>("Rate", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(1.0);
            Ok(())
        });

    let property_volume = f
        .property::<f64, _>("Volume", ())
        .access(Access::Read)
        .on_get(spotify_api_property!([sp, _device]
            if let Ok(Some(player)) = sp.current_playback(None) {
                player.device.volume_percent as f64
            } else {
                0.0
            }
        ));

    let property_max_rate = f
        .property::<f64, _>("MaximumRate", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(1.0);
            Ok(())
        });

    let property_min_rate = f
        .property::<f64, _>("MinimumRate", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(1.0);
            Ok(())
        });

    let property_loop_status = f
        .property::<String, _>("LoopStatus", ())
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
        ));

    let property_position = f
        .property::<i64, _>("Position", ())
        .access(Access::Read)
        .on_get(spotify_api_property!([sp, _device]
            if let Ok(Some(pos)) =
                sp.current_playback(None)
                .map(|maybe_player| maybe_player.and_then(|p| p.progress_ms)) {
                i64::from(pos) * 1000
            } else {
                0
            }
        ));

    let property_metadata = f
        .property::<HashMap<String, Variant<Box<dyn RefArg>>>, _>("Metadata", ())
        .access(Access::Read)
        .on_get(spotify_api_property!([sp, _device] {
            let mut m = HashMap::new();
            let v = sp.current_user_playing_track();

            if let Ok(Some(playing)) = v {
                if let Some(track) = playing.item {
                    m.insert("mpris:trackid".to_string(), Variant(Box::new(
                        MessageItem::Str(
                            track.uri
                        )) as Box<dyn RefArg>));

                    m.insert("mpris:length".to_string(), Variant(Box::new(
                        MessageItem::Int64(
                            i64::from(track.duration_ms) * 1000
                        )) as Box<dyn RefArg>));

                    m.insert("mpris:artUrl".to_string(), Variant(Box::new(
                        MessageItem::Str(
                            track.album.images
                                .first()
                                .unwrap().url.clone()
                        )) as Box<dyn RefArg>));

                    m.insert("xesam:title".to_string(), Variant(Box::new(
                        MessageItem::Str(
                            track.name
                        )) as Box<dyn RefArg>));

                    m.insert("xesam:album".to_string(), Variant(Box::new(
                        MessageItem::Str(
                            track.album.name
                        )) as Box<dyn RefArg>));

                    m.insert("xesam:artist".to_string(), Variant(Box::new(
                        MessageItem::Array(MessageItemArray::new(
                            track.artists
                                .iter()
                                .map(|a| MessageItem::Str(a.name.to_string()))
                                .collect::<Vec<_>>(), Signature::new("as").unwrap()
                        ).unwrap())) as Box<dyn RefArg>));

                    m.insert("xesam:albumArtist".to_string(), Variant(Box::new(
                        MessageItem::Array(MessageItemArray::new(
                            track.album.artists
                                .iter()
                                .map(|a| MessageItem::Str(a.name.to_string()))
                                .collect::<Vec<_>>(), Signature::new("as").unwrap()
                        ).unwrap())) as Box<dyn RefArg>));

                    m.insert("xesam:autoRating".to_string(), Variant(Box::new(
                        MessageItem::Double(
                            f64::from(track.popularity) / 100.0
                        )) as Box<dyn RefArg>));

                    m.insert("xesam:trackNumber".to_string(), Variant(Box::new(
                        MessageItem::UInt32(
                            track.track_number
                        )) as Box<dyn RefArg>));

                    m.insert("xesam:discNumber".to_string(), Variant(Box::new(
                        MessageItem::Int32(
                            track.disc_number
                        )) as Box<dyn RefArg>));

                    m.insert("xesam:url".to_string(), Variant(Box::new(
                        MessageItem::Str(
                            track.external_urls
                                .iter()
                                .next()
                                .map_or("", |(_, v)| &v)
                                .to_string()
                        )) as Box<dyn RefArg>));
                }
            } else {
                info!("Couldn't fetch metadata from spotify: {:?}", v);
            }

            m
        }));

    let property_can_play = f
        .property::<bool, _>("CanPlay", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(true);
            Ok(())
        });

    let property_can_pause = f
        .property::<bool, _>("CanPause", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(true);
            Ok(())
        });

    let property_can_seek = f
        .property::<bool, _>("CanSeek", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(true);
            Ok(())
        });

    let property_can_control = f
        .property::<bool, _>("CanControl", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(true);
            Ok(())
        });

    let property_can_go_previous = f
        .property::<bool, _>("CanGoPrevious", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(true);
            Ok(())
        });

    let property_can_go_next = f
        .property::<bool, _>("CanGoNext", ())
        .access(Access::Read)
        .on_get(|iter, _| {
            iter.append(true);
            Ok(())
        });

    let media_player2_player_interface = f
        .interface("org.mpris.MediaPlayer2.Player", ())
        .add_m(method_volume_up)
        .add_m(method_volume_down)
        .add_m(method_next)
        .add_m(method_previous)
        .add_m(method_pause)
        .add_m(method_play_pause)
        .add_m(method_play)
        .add_m(method_stop)
        .add_m(method_seek)
        .add_m(method_set_position)
        .add_m(method_open_uri)
        .add_p(property_playback_status)
        .add_p(property_rate)
        .add_p(property_volume)
        .add_p(property_max_rate)
        .add_p(property_min_rate)
        .add_p(property_loop_status)
        .add_p(property_position)
        .add_p(property_metadata)
        .add_p(property_can_play)
        .add_p(property_can_pause)
        .add_p(property_can_seek)
        .add_p(property_can_control)
        .add_p(property_can_go_next)
        .add_p(property_can_go_previous)
        .add_p(property_shuffle);

    let tree = f.tree(ATree::new()).add(
        f.object_path("/org/mpris/MediaPlayer2", ())
            .introspectable()
            .add(media_player2_interface)
            .add(media_player2_player_interface),
    );

    tree.set_registered(&connection, true)
        .expect("Failed to register tree");

    //let handle = tokio_compat::runtime::Handle::current();

    let async_connection = AConnection::new(connection.clone(), handle)
        .expect("Failed to create async dbus connection");

    let server = ATreeServer::new(
        connection,
        Box::new(tree),
        async_connection
            .messages()
            .expect("Failed to unwrap async messages"),
    );

    Box::pin(server.for_each(|message| {
        warn!("Unhandled DBus message: {:?}", message);
        Ok(())
    }))
}

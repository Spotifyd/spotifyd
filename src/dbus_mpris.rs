use chrono::prelude::*;
use dbus::arg::{RefArg, Variant};
use dbus::channel::{MatchingReceiver, Sender};
use dbus::message::{MatchRule, SignalArgs};
use dbus_crossroads::{Crossroads, IfaceToken};
use dbus_tokio::connection;
use futures::task::{Context, Poll};
use futures::{self, Future};
use librespot_connect::spirc::Spirc;
use librespot_core::{
    keymaster::{get_token, Token as LibrespotToken},
    mercury::MercuryError,
    session::Session,
};
use librespot_playback::player::PlayerEvent;
use log::info;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rspotify::spotify::{
    client::Spotify,
    model::{offset::for_position, track::FullTrack},
    oauth2::TokenInfo as RspotifyToken,
    senum::*,
    util::datetime_to_timestamp,
};
use std::pin::Pin;
use std::sync::Arc;
use std::{collections::HashMap, env};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct DbusServer {
    session: Session,
    spirc: Arc<Spirc>,
    api_token: RspotifyToken,
    #[allow(clippy::type_complexity)]
    token_request: Option<Pin<Box<dyn Future<Output = Result<LibrespotToken, MercuryError>>>>>,
    dbus_future: Option<Pin<Box<dyn Future<Output = ()>>>>,
    device_name: String,
    event_rx: UnboundedReceiver<PlayerEvent>,
    event_tx: Option<UnboundedSender<PlayerEvent>>,
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
        spirc: Arc<Spirc>,
        device_name: String,
        event_rx: UnboundedReceiver<PlayerEvent>,
    ) -> DbusServer {
        DbusServer {
            session,
            spirc,
            api_token: RspotifyToken::default(),
            token_request: None,
            dbus_future: None,
            device_name,
            event_rx,
            event_tx: None,
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
        if self.event_tx.is_some() {
            if let Poll::Ready(Some(msg)) = self.event_rx.poll_recv(cx) {
                self.event_tx.as_ref().unwrap().send(msg).unwrap();
            }
        }
        let mut got_new_token = false;
        if self.is_token_expired() {
            if let Some(ref mut fut) = self.token_request {
                if let Poll::Ready(Ok(token)) = fut.as_mut().poll(cx) {
                    self.api_token = RspotifyToken::default()
                        .access_token(&token.access_token)
                        .expires_in(token.expires_in)
                        .expires_at(datetime_to_timestamp(token.expires_in));
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                    self.event_tx = Some(tx);
                    self.dbus_future = Some(Box::pin(create_dbus_server(
                        self.api_token.clone(),
                        self.spirc.clone(),
                        self.device_name.clone(),
                        rx,
                    )));
                    // TODO: for reasons I don't _entirely_ understand, the token request completing
                    // convinces callers that they don't need to re-check the status of this future
                    // until we start playing. This causes DBUS to not respond until that point in
                    // time. So, fire a "wake" here, which tells callers to keep checking.
                    cx.waker().clone().wake();
                    got_new_token = true;
                }
            } else {
                self.token_request = Some(Box::pin({
                    let sess = self.session.clone();
                    // This is more meant as a fast hotfix than anything else!
                    let client_id =
                        env::var("SPOTIFYD_CLIENT_ID").unwrap_or_else(|_| CLIENT_ID.to_string());
                    async move { get_token(&sess, &client_id, SCOPE).await }
                }));
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

fn create_spotify_api(token: &RspotifyToken) -> Spotify {
    Spotify::default().access_token(&token.access_token).build()
}

async fn create_dbus_server(
    api_token: RspotifyToken,
    spirc: Arc<Spirc>,
    device_name: String,
    mut event_rx: UnboundedReceiver<PlayerEvent>,
) {
    // TODO: allow other DBus types through CLI and config entry.
    let (resource, conn) =
        connection::new_session_sync().expect("Failed to initialize DBus connection");
    tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });

    conn.request_name("org.mpris.MediaPlayer2.spotifyd", false, true, true)
        .await
        .expect("Failed to register dbus player name");

    let mut cr = Crossroads::new();
    cr.set_async_support(Some((
        conn.clone(),
        Box::new(|x| {
            tokio::spawn(x);
        }),
    )));

    // The following methods and properties are part of the MediaPlayer2 interface.
    // https://specifications.freedesktop.org/mpris-spec/latest/Media_Player.html
    let media_player2_interface = cr.register("org.mpris.MediaPlayer2", |b| {
        b.method("Raise", (), (), move |_, _, (): ()| {
            // noop
            Ok(())
        });
        let local_spirc = spirc.clone();
        b.method("Quit", (), (), move |_, _, (): ()| {
            local_spirc.shutdown();
            Ok(())
        });
        b.property("CanQuit")
            .emits_changed_const()
            .get(|_, _| Ok(true));
        b.property("CanRaise")
            .emits_changed_const()
            .get(|_, _| Ok(false));
        b.property("CanSetFullscreen")
            .emits_changed_const()
            .get(|_, _| Ok(false));
        b.property("HasTrackList")
            .emits_changed_const()
            .get(|_, _| Ok(false));
        b.property("Identity")
            .emits_changed_const()
            .get(|_, _| Ok("Spotifyd".to_string()));
        b.property("SupportedUriSchemes")
            .emits_changed_const()
            .get(|_, _| Ok(vec!["spotify".to_string()]));
        b.property("SupportedMimeTypes")
            .emits_changed_const()
            .get(|_, _| Ok(Vec::<String>::new()));
    });

    // The following methods and properties are part of the MediaPlayer2.Player interface.
    // https://specifications.freedesktop.org/mpris-spec/latest/Player_Interface.html

    let player_interface: IfaceToken<()> = cr.register("org.mpris.MediaPlayer2.Player", |b| {
        let local_spirc = spirc.clone();
        b.method("VolumeUp", (), (), move |_, _, (): ()| {
            local_spirc.volume_up();
            Ok(())
        });
        let local_spirc = spirc.clone();
        b.method("VolumeDown", (), (), move |_, _, (): ()| {
            local_spirc.volume_down();
            Ok(())
        });
        let local_spirc = spirc.clone();
        b.method("Next", (), (), move |_, _, (): ()| {
            local_spirc.next();
            Ok(())
        });
        let local_spirc = spirc.clone();
        b.method("Previous", (), (), move |_, _, (): ()| {
            local_spirc.prev();
            Ok(())
        });
        let local_spirc = spirc.clone();
        b.method("Pause", (), (), move |_, _, (): ()| {
            local_spirc.pause();
            Ok(())
        });
        let local_spirc = spirc.clone();
        b.method("PlayPause", (), (), move |_, _, (): ()| {
            local_spirc.play_pause();
            Ok(())
        });
        let local_spirc = spirc.clone();
        b.method("Play", (), (), move |_, _, (): ()| {
            local_spirc.play();
            Ok(())
        });
        let local_spirc = spirc.clone();
        b.method("Stop", (), (), move |_, _, (): ()| {
            // TODO: add real stop implementation.
            local_spirc.pause();
            Ok(())
        });

        let mv_device_name = device_name.clone();
        let mv_api_token = api_token.clone();
        b.method("Seek", ("pos",), (), move |_, _, (pos,): (u32,)| {
            let device_name = utf8_percent_encode(&mv_device_name, NON_ALPHANUMERIC).to_string();
            let sp = create_spotify_api(&mv_api_token);
            if let Ok(Some(playing)) = sp.current_user_playing_track() {
                let _ = sp.seek_track(playing.progress_ms.unwrap_or(0) + pos, Some(device_name));
            }
            Ok(())
        });

        let mv_device_name = device_name.clone();
        let mv_api_token = api_token.clone();
        b.method("SetPosition", ("pos",), (), move |_, _, (pos,): (u32,)| {
            let device_name = utf8_percent_encode(&mv_device_name, NON_ALPHANUMERIC).to_string();
            let sp = create_spotify_api(&mv_api_token);
            let _ = sp.seek_track(pos, Some(device_name));
            Ok(())
        });

        let mv_device_name = device_name.clone();
        let mv_api_token = api_token.clone();
        b.method("OpenUri", ("uri",), (), move |_, _, (uri,): (String,)| {
            let device_name = utf8_percent_encode(&mv_device_name, NON_ALPHANUMERIC).to_string();
            let sp = create_spotify_api(&mv_api_token);
            let device_id = match sp.device() {
                Ok(device_payload) => {
                    match device_payload
                        .devices
                        .into_iter()
                        .find(|d| d.is_active && d.name == device_name)
                    {
                        Some(device) => Some(device.id),
                        None => None,
                    }
                }
                Err(_) => None,
            };

            if uri.contains("spotify:track") {
                let _ = sp.start_playback(device_id, None, Some(vec![uri]), for_position(0), None);
            } else {
                let _ = sp.start_playback(device_id, Some(uri), None, for_position(0), None);
            }
            Ok(())
        });

        let mv_device_name = device_name.clone();
        let mv_api_token = api_token.clone();
        b.property("PlaybackStatus")
            .emits_changed_false()
            .get(move |_, _| {
                let sp = create_spotify_api(&mv_api_token);
                if let Ok(Some(player)) = sp.current_playback(None) {
                    if player.device.name == mv_device_name {
                        if let Ok(Some(track)) = sp.current_user_playing_track() {
                            if track.is_playing {
                                return Ok("Playing".to_string());
                            } else {
                                return Ok("Paused".to_string());
                            }
                        }
                    }
                }
                Ok("Stopped".to_string())
            });

        let mv_api_token = api_token.clone();
        b.property("Shuffle")
            .emits_changed_false()
            .get(move |_, _| {
                let sp = create_spotify_api(&mv_api_token);
                let shuffle_status = sp
                    .current_playback(None)
                    .ok()
                    .flatten()
                    .map_or(false, |p| p.shuffle_state);
                Ok(shuffle_status)
            });

        b.property("Rate").emits_changed_const().get(|_, _| Ok(1.0));

        let mv_api_token = api_token.clone();
        b.property("Volume").emits_changed_false().get(move |_, _| {
            let sp = create_spotify_api(&mv_api_token);
            let vol = sp
                .current_playback(None)
                .ok()
                .flatten()
                .map_or(0.0, |p| p.device.volume_percent as f64);
            Ok(vol)
        });

        b.property("MaximumRate")
            .emits_changed_const()
            .get(|_, _| Ok(1.0));
        b.property("MinimumRate")
            .emits_changed_const()
            .get(|_, _| Ok(1.0));

        let mv_api_token = api_token.clone();
        b.property("LoopStatus")
            .emits_changed_false()
            .get(move |_, _| {
                let sp = create_spotify_api(&mv_api_token);
                let status = if let Ok(Some(player)) = sp.current_playback(None) {
                    match player.repeat_state {
                        RepeatState::Off => "None",
                        RepeatState::Track => "Track",
                        RepeatState::Context => "Playlist",
                    }
                } else {
                    "None"
                }
                .to_string();
                Ok(status)
            });

        let mv_api_token = api_token.clone();
        b.property("Position")
            .emits_changed_false()
            .get(move |_, _| {
                let sp = create_spotify_api(&mv_api_token);
                let val = if let Ok(Some(pos)) = sp
                    .current_playback(None)
                    .map(|maybe_player| maybe_player.and_then(|p| p.progress_ms))
                {
                    i64::from(pos) * 1000
                } else {
                    0
                };
                Ok(val)
            });

        let mv_api_token = api_token.clone();
        b.property("Metadata")
            .emits_changed_false()
            .get(move |_, _| {
                let sp = create_spotify_api(&mv_api_token);

                let mut m: HashMap<String, Variant<Box<dyn RefArg>>> = HashMap::new();
                let v = sp.current_user_playing_track();

                if let Ok(Some(playing)) = v {
                    if let Some(track) = playing.item {
                        insert_metadata(&mut m, track);
                    }
                } else {
                    info!("Couldn't fetch metadata from spotify: {:?}", v);
                }

                Ok(m)
            });

        for prop in &[
            "CanPlay",
            "CanPause",
            "CanSeek",
            "CanControl",
            "CanGoPrevious",
            "CanGoNext",
        ] {
            b.property(*prop).emits_changed_const().get(|_, _| Ok(true));
        }
    });

    cr.insert(
        "/org/mpris/MediaPlayer2",
        &[media_player2_interface, player_interface],
        (),
    );

    conn.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |msg, conn| {
            cr.handle_message(msg, conn).unwrap();
            true
        }),
    );

    // Store current playback state to be able to detect changes
    let mut last_track_id = None;
    let mut last_playback_status = None;
    let mut last_volume = None;

    loop {
        let event = event_rx
            .recv()
            .await
            .expect("Changed track channel was unexpectedly closed");
        let mut seeked_position = None;

        // Update playback state from event
        let (track_id, playback_status, player_volume) = match event {
            PlayerEvent::VolumeSet { volume } => {
                (last_track_id, last_playback_status, Some(volume))
            }
            PlayerEvent::Playing { track_id, position_ms, .. } => {
                seeked_position = Some(position_ms);
                (Some(track_id), Some(PlaybackStatus::Playing), last_volume)
            }
            PlayerEvent::Stopped { .. } => {
                (last_track_id, Some(PlaybackStatus::Stopped), last_volume)
            }
            PlayerEvent::Paused { .. } => {
                (last_track_id, Some(PlaybackStatus::Paused), last_volume)
            }
            _ => (last_track_id, last_playback_status, last_volume),
        };

        // if playback_status, track_id or volume have changed, emit a PropertiesChanged signal
        if last_playback_status != playback_status || last_track_id != track_id || last_volume != player_volume {
            let mut changed_properties: HashMap<String, Variant<Box<dyn RefArg>>> = HashMap::new();

            if last_volume != player_volume {
                if let Some(player_volume) = player_volume {
                    // convert u16 to float
                    let mut vol_mpris = player_volume as f64;
                    // max. vol = 1.0 according to mpris spec, round to two decimal places
                    vol_mpris = (vol_mpris / 65535.0 * 100.0).round() / 100.0;
                    changed_properties.insert("Volume".to_owned(), Variant(Box::new(vol_mpris.to_owned())));
                }
            }
            else {
                if let Some(track_id) = track_id {
                    let sp = create_spotify_api(&api_token);
                    let track = sp.track(&track_id.to_base62());
                    if let Ok(track) = track {
                        let mut m: HashMap<String, Variant<Box<dyn RefArg>>> = HashMap::new();
                        insert_metadata(&mut m, track);

                        changed_properties.insert("Metadata".to_owned(), Variant(Box::new(m)));
                    } else {
                        info!("Couldn't fetch metadata from spotify: {:?}", track);
                    }
                }
                if let Some(playback_status) = playback_status {
                    changed_properties.insert("PlaybackStatus".to_owned(), Variant(Box::new(match playback_status {
                        PlaybackStatus::Playing => "Playing".to_owned(),
                        PlaybackStatus::Paused => "Paused".to_owned(),
                        PlaybackStatus::Stopped => "Stopped".to_owned(),
                    })));
                }
            }

            let msg = dbus::nonblock::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged {
                interface_name: "org.mpris.MediaPlayer2.Player".to_owned(),
                changed_properties,
                invalidated_properties: Vec::new(),
            };
            conn.send(msg.to_emit_message(&dbus::Path::new("/org/mpris/MediaPlayer2").unwrap()))
                .unwrap();

            last_playback_status = playback_status;
            last_track_id = track_id;
            last_volume = player_volume;
        }

        // if position in track has changed emit a Seeked signal
        if let Some(position) = seeked_position {
            let msg = dbus::message::Message::signal(
                &dbus::Path::new("/org/mpris/MediaPlayer2").unwrap(),
                &dbus::strings::Interface::new("org.mpris.MediaPlayer2.Player").unwrap(),
                &dbus::strings::Member::new("Seeked").unwrap(),
            )
            .append1(position as i64);
            conn.send(msg).unwrap();
        }
    }
}

fn insert_metadata(m: &mut HashMap<String, Variant<Box<dyn RefArg>>>, track: FullTrack) {
    m.insert("mpris:trackid".to_string(), Variant(Box::new(track.uri)));

    m.insert(
        "mpris:length".to_string(),
        Variant(Box::new(i64::from(track.duration_ms) * 1000)),
    );

    m.insert(
        "mpris:artUrl".to_string(),
        Variant(Box::new(track.album.images.first().unwrap().url.clone())),
    );

    m.insert("xesam:title".to_string(), Variant(Box::new(track.name)));

    m.insert(
        "xesam:album".to_string(),
        Variant(Box::new(track.album.name)),
    );

    m.insert(
        "xesam:artist".to_string(),
        Variant(Box::new(
            track
                .artists
                .iter()
                .map(|a| a.name.to_string())
                .collect::<Vec<_>>(),
        )),
    );

    m.insert(
        "xesam:albumArtist".to_string(),
        Variant(Box::new(
            track
                .album
                .artists
                .iter()
                .map(|a| a.name.to_string())
                .collect::<Vec<_>>(),
        )),
    );

    m.insert(
        "xesam:autoRating".to_string(),
        Variant(Box::new((f64::from(track.popularity) / 100.0) as f64)),
    );

    m.insert(
        "xesam:trackNumber".to_string(),
        Variant(Box::new(track.track_number)),
    );

    m.insert(
        "xesam:discNumber".to_string(),
        Variant(Box::new(track.disc_number)),
    );

    m.insert(
        "xesam:url".to_string(),
        Variant(Box::new(
            track
                .external_urls
                .iter()
                .next()
                .map_or("", |(_, v)| &v)
                .to_string(),
        )),
    );
}

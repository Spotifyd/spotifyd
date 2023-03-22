use crate::config::DBusType;
use chrono::{prelude::*, Duration};
use dbus::{
    arg::{RefArg, Variant},
    channel::{MatchingReceiver, Sender},
    message::{MatchRule, SignalArgs},
    MethodErr,
};
use dbus_crossroads::{Crossroads, IfaceToken};
use dbus_tokio::connection;
use futures::{
    self,
    task::{Context, Poll},
    Future,
};
use librespot_connect::spirc::Spirc;
use librespot_core::{
    keymaster::{get_token, Token as LibrespotToken},
    mercury::MercuryError,
    session::Session,
    spotify_id::SpotifyAudioType,
};
use librespot_playback::player::PlayerEvent;
use log::{error, info, warn};
use rspotify::{
    model::{
        offset::Offset, parse_uri, AlbumId, ArtistId, EpisodeId, IdError, PlayableItem, PlaylistId,
        RepeatState, ShowId, TrackId, Type,
    },
    prelude::*,
    AuthCodeSpotify, Token as RspotifyToken,
};
use std::{collections::HashMap, convert::TryInto, env, pin::Pin, sync::Arc};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct DbusServer {
    session: Session,
    spirc: Arc<Spirc>,
    spotify_client: Arc<AuthCodeSpotify>,
    dbus_type: DBusType,
    #[allow(clippy::type_complexity)]
    token_request: Option<Pin<Box<dyn Future<Output = Result<LibrespotToken, MercuryError>>>>>,
    dbus_future: Option<Pin<Box<dyn Future<Output = ()>>>>,
    device_name: String,
    event_rx: UnboundedReceiver<PlayerEvent>,
    event_tx: Option<UnboundedSender<PlayerEvent>>,
}

const CLIENT_ID: &str = "2c1ea588dfbc4a989e2426f8385297c3";
const SCOPE: &str =
    "user-read-playback-state,user-modify-playback-state,user-read-currently-playing";

impl DbusServer {
    pub fn new(
        session: Session,
        spirc: Arc<Spirc>,
        device_name: String,
        event_rx: UnboundedReceiver<PlayerEvent>,
        dbus_type: DBusType,
    ) -> DbusServer {
        DbusServer {
            session,
            spirc,
            spotify_client: Default::default(),
            dbus_type,
            token_request: None,
            dbus_future: None,
            device_name,
            event_rx,
            event_tx: None,
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
        let needs_token = match *self.spotify_client.get_token().lock().unwrap() {
            Some(ref token) => token.is_expired(),
            None => true,
        };

        if needs_token {
            if let Some(mut fut) = self.token_request.take() {
                if let Poll::Ready(token) = fut.as_mut().poll(cx) {
                    let token = match token {
                        Ok(token) => token,
                        Err(_) => {
                            error!("failed to request a token for the web API");
                            // shutdown DBus-Server
                            return Poll::Ready(());
                        }
                    };

                    let expires_in = Duration::seconds(token.expires_in as i64);
                    let api_token = RspotifyToken {
                        access_token: token.access_token,
                        expires_in,
                        expires_at: Some(Utc::now() + expires_in),
                        ..RspotifyToken::default()
                    };

                    if self.dbus_future.is_none() {
                        self.spotify_client = Arc::new(AuthCodeSpotify::from_token(api_token));

                        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                        self.event_tx = Some(tx);
                        self.dbus_future = Some(Box::pin(create_dbus_server(
                            Arc::clone(&self.spotify_client),
                            self.spirc.clone(),
                            self.device_name.clone(),
                            rx,
                            self.dbus_type,
                        )));
                    } else {
                        *self.spotify_client.get_token().lock().unwrap() = Some(api_token);
                    }
                } else {
                    self.token_request = Some(fut);
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
        }

        // Not polling the future here in some cases is fine, since we will poll it
        // immediately after the token request has completed.
        // If we would poll the future in any case, we would risk using invalid tokens for API requests.
        if self.token_request.is_none() {
            if let Some(ref mut fut) = self.dbus_future {
                return fut.as_mut().poll(cx);
            }
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

async fn create_dbus_server(
    spotify_api_client: Arc<AuthCodeSpotify>,
    spirc: Arc<Spirc>,
    device_name: String,
    mut event_rx: UnboundedReceiver<PlayerEvent>,
    dbus_type: DBusType,
) {
    let (resource, conn) = match dbus_type {
        DBusType::Session => connection::new_session_sync(),
        DBusType::System => connection::new_system_sync(),
    }
    .expect("Failed to initialize DBus connection");
    tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });

    let path = format!(
        "org.mpris.MediaPlayer2.spotifyd.instance{}",
        std::process::id()
    );

    // TODO: The first `true` allows us to replace orphaned dbus servers from previous sessions
    // later. We should instead properly release the name when the session ends.
    conn.request_name(path, true, true, true)
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
        })
        .deprecated();
        let local_spirc = spirc.clone();
        b.method("VolumeDown", (), (), move |_, _, (): ()| {
            local_spirc.volume_down();
            Ok(())
        })
        .deprecated();
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
        let sp_client = Arc::clone(&spotify_api_client);
        b.method("Seek", ("offset",), (), move |_, _, (pos,): (i64,)| {
            if let Ok(Some(playback)) = sp_client.current_playback(None, None::<Vec<_>>) {
                if playback.device.name == mv_device_name {
                    let new_pos = playback
                        .progress
                        .and_then(|d| d.as_millis().try_into().ok())
                        .and_then(|d: i64| d.checked_add(pos / 1000));

                    if let Some(new_pos) = new_pos {
                        let duration: u32 = match playback.item {
                            Some(PlayableItem::Track(t)) => t.duration.as_millis(),
                            Some(PlayableItem::Episode(e)) => e.duration.as_millis(),
                            None => return Ok(()),
                        }
                        .try_into()
                        .unwrap_or(u32::MAX);

                        // MPRIS spec: negative values should be treated as 0
                        let new_pos = new_pos.max(0);
                        if new_pos <= duration as i64 {
                            let _ =
                                sp_client.seek_track(new_pos as u32, playback.device.id.as_deref());
                        } else {
                            // MPRIS spec: values beyond track bounds should act like Next
                            let _ = sp_client.next_track(playback.device.id.as_deref());
                        }
                    }
                }
            }
            Ok(())
        });

        let mv_device_name = device_name.clone();
        let sp_client = Arc::clone(&spotify_api_client);
        b.method(
            "SetPosition",
            ("track_id", "position"),
            (),
            move |_, _, (track_id, pos): (dbus::Path, i64)| {
                if let Ok(Some(playback)) = sp_client.current_playback(None, None::<Vec<_>>) {
                    let (track_matches, duration) = if let Some(item) = playback.item {
                        let track_matches = item
                            .id()
                            .map(|id| uri_to_object_path(id.uri()) == track_id)
                            .unwrap_or(false);
                        let duration = match item {
                            PlayableItem::Track(t) => t.duration.as_micros(),
                            PlayableItem::Episode(e) => e.duration.as_micros(),
                        }
                        .try_into()
                        .unwrap_or(i64::MAX);
                        (track_matches, duration)
                    } else {
                        return Ok(());
                    };

                    // as per MPRIS spec: pos must be in track bounds and track id must match
                    if playback.device.name == mv_device_name
                        && track_matches
                        && (0..=duration).contains(&pos)
                    {
                        // pos is in microseconds, seek_track takes milliseconds
                        let _ = sp_client
                            .seek_track((pos / 1000) as u32, playback.device.id.as_deref());
                    }
                }
                Ok(())
            },
        );

        let mv_device_name = device_name.clone();
        let sp_client = Arc::clone(&spotify_api_client);
        b.method("OpenUri", ("uri",), (), move |_, _, (uri,): (String,)| {
            enum AnyId<'a> {
                Playable(PlayableId<'a>),
                Context(PlayContextId<'a>),
            }

            fn uri_to_id(uri: &str) -> Result<AnyId<'_>, IdError> {
                use AnyId::*;
                Ok(match parse_uri(uri)? {
                    (Type::Track, id) => Playable(TrackId::from_id(id)?.into()),
                    (Type::Episode, id) => Playable(EpisodeId::from_id(id)?.into()),
                    (Type::Artist, id) => Context(ArtistId::from_id(id)?.into()),
                    (Type::Album, id) => Context(AlbumId::from_id(id)?.into()),
                    (Type::Playlist, id) => Context(PlaylistId::from_id(id)?.into()),
                    (Type::Show, id) => Context(ShowId::from_id(id)?.into()),
                    (Type::User | Type::Collection, _) => Err(IdError::InvalidType)?,
                })
            }

            let id = uri_to_id(&uri).map_err(|e| MethodErr::invalid_arg(&e))?;

            let device_id = get_device_id(&sp_client, &mv_device_name, true);

            if let Some(device_id) = device_id {
                match id {
                    AnyId::Playable(id) => {
                        let _ = sp_client.start_uris_playback(
                            Some(id),
                            Some(&device_id),
                            Some(Offset::Position(0)),
                            None,
                        );
                    }
                    AnyId::Context(id) => {
                        let _ = sp_client.start_context_playback(
                            id,
                            Some(&device_id),
                            Some(Offset::Position(0)),
                            None,
                        );
                    }
                }
            }
            Ok(())
        });

        let mv_device_name = device_name.clone();
        let sp_client = Arc::clone(&spotify_api_client);
        b.property("PlaybackStatus")
            .emits_changed_false()
            .get(move |_, _| {
                if let Ok(Some(playback)) = sp_client.current_playback(None, None::<Vec<_>>) {
                    if playback.device.name == mv_device_name {
                        if playback.is_playing {
                            return Ok("Playing".to_string());
                        } else {
                            return Ok("Paused".to_string());
                        }
                    }
                }
                Ok("Stopped".to_string())
            });

        let sp_client = Arc::clone(&spotify_api_client);
        b.property("Shuffle")
            .emits_changed_false()
            .get(move |_, _| {
                let shuffle_status = sp_client
                    .current_playback(None, None::<Vec<_>>)
                    .ok()
                    .flatten()
                    .map_or(false, |p| p.shuffle_state);
                Ok(shuffle_status)
            });

        b.property("Rate").emits_changed_const().get(|_, _| Ok(1.0));

        let sp_client = Arc::clone(&spotify_api_client);
        b.property("Volume").emits_changed_false().get(move |_, _| {
            let vol = sp_client
                .current_playback(None, None::<Vec<_>>)
                .ok()
                .flatten()
                .and_then(|p| p.device.volume_percent)
                .unwrap_or(0) as f64;

            Ok(vol)
        });

        b.property("MaximumRate")
            .emits_changed_const()
            .get(|_, _| Ok(1.0));
        b.property("MinimumRate")
            .emits_changed_const()
            .get(|_, _| Ok(1.0));

        let sp_client = Arc::clone(&spotify_api_client);
        b.property("LoopStatus")
            .emits_changed_false()
            .get(move |_, _| {
                let status =
                    if let Ok(Some(player)) = sp_client.current_playback(None, None::<Vec<_>>) {
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

        let sp_client = Arc::clone(&spotify_api_client);
        b.property("Position")
            .emits_changed_false()
            .get(move |_, _| {
                let pos = sp_client
                    .current_playback(None, None::<Vec<_>>)
                    .ok()
                    .flatten()
                    .and_then(|p| Some(p.progress?.as_micros() as i64))
                    .unwrap_or(0);

                Ok(pos)
            });

        let sp_client = Arc::clone(&spotify_api_client);
        b.property("Metadata")
            .emits_changed_false()
            .get(move |_, _| {
                let mut m: HashMap<String, Variant<Box<dyn RefArg>>> = HashMap::new();
                let item = match sp_client.current_playing(None, None::<Vec<_>>) {
                    Ok(playing) => playing.and_then(|playing| playing.item),
                    Err(e) => {
                        info!("Couldn't fetch metadata from spotify: {:?}", e);
                        return Ok(m);
                    }
                };

                if let Some(item) = item {
                    insert_metadata(&mut m, item);
                } else {
                    info!("Couldn't fetch metadata from spotify: Nothing playing at the moment.");
                }

                Ok(m)
            });

        for prop in [
            "CanPlay",
            "CanPause",
            "CanSeek",
            "CanControl",
            "CanGoPrevious",
            "CanGoNext",
        ] {
            b.property(prop).emits_changed_const().get(|_, _| Ok(true));
        }
    });

    let spotifyd_ctrls_interface: IfaceToken<()> =
        cr.register("io.github.spotifyd.Controls", |b| {
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

            let mv_device_name = device_name.clone();
            let sp_client = Arc::clone(&spotify_api_client);
            b.method("TransferPlayback", (), (), move |_, _, (): ()| {
                let device_id = get_device_id(&sp_client, &mv_device_name, false);
                if let Some(device_id) = device_id {
                    info!("Transferring playback to device {}", device_id);
                    match sp_client.transfer_playback(&device_id, Some(true)) {
                        Ok(_) => Ok(()),
                        Err(err) => {
                            let e = format!("TransferPlayback failed: {}", err);
                            error!("{}", e);
                            Err(MethodErr::failed(&e))
                        }
                    }
                } else {
                    let msg = format!("Could not find device with name {}", mv_device_name);
                    warn!("TransferPlayback: {}", msg);
                    Err(MethodErr::failed(&msg))
                }
            });
        });

    cr.insert(
        "/org/mpris/MediaPlayer2",
        &[media_player2_interface, player_interface],
        (),
    );

    cr.insert(
        "/io/github/spotifyd/Controls",
        &[spotifyd_ctrls_interface],
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
        let mut seeked_position_ms = None;

        // Update playback state from event
        let (track_id, playback_status, player_volume) = match event {
            PlayerEvent::VolumeSet { volume } => {
                (last_track_id, last_playback_status, Some(volume))
            }
            PlayerEvent::Playing {
                track_id,
                position_ms,
                ..
            } => {
                seeked_position_ms = Some(position_ms);
                (Some(track_id), Some(PlaybackStatus::Playing), last_volume)
            }
            PlayerEvent::Stopped { .. } => {
                (last_track_id, Some(PlaybackStatus::Stopped), last_volume)
            }
            PlayerEvent::Paused { .. } => {
                (last_track_id, Some(PlaybackStatus::Paused), last_volume)
            }
            _ => continue,
        };

        // if playback_status, track_id or volume have changed, emit a PropertiesChanged signal
        if last_playback_status != playback_status
            || last_track_id != track_id
            || last_volume != player_volume
        {
            let mut changed_properties: HashMap<String, Variant<Box<dyn RefArg>>> = HashMap::new();

            if last_volume != player_volume {
                if let Some(player_volume) = player_volume {
                    // convert u16 to float
                    let mut vol_mpris = player_volume as f64;
                    // max. vol = 1.0 according to mpris spec, round to two decimal places
                    vol_mpris = (vol_mpris / 65535.0 * 100.0).round() / 100.0;
                    changed_properties
                        .insert("Volume".to_owned(), Variant(Box::new(vol_mpris.to_owned())));
                }
            } else {
                if let Some(track_id) = track_id {
                    let item = match track_id.audio_type {
                        SpotifyAudioType::Track => {
                            let track_id = TrackId::from_id(track_id.to_base62()).unwrap();
                            let track = spotify_api_client.track(track_id).map(PlayableItem::Track);
                            Some(track)
                        }
                        SpotifyAudioType::Podcast => {
                            let id = EpisodeId::from_id(track_id.to_base62()).unwrap();
                            let episode = spotify_api_client
                                .get_an_episode(id, None)
                                .map(PlayableItem::Episode);
                            Some(episode)
                        }
                        SpotifyAudioType::NonPlayable => None,
                    };

                    if let Some(item) = item {
                        match item {
                            Ok(item) => {
                                let mut m: HashMap<String, Variant<Box<dyn RefArg>>> =
                                    HashMap::new();
                                insert_metadata(&mut m, item);

                                changed_properties
                                    .insert("Metadata".to_owned(), Variant(Box::new(m)));
                            }
                            Err(e) => info!("Couldn't fetch metadata from spotify: {:?}", e),
                        }
                    }
                }
                if let Some(playback_status) = playback_status {
                    changed_properties.insert(
                        "PlaybackStatus".to_owned(),
                        Variant(Box::new(match playback_status {
                            PlaybackStatus::Playing => "Playing".to_owned(),
                            PlaybackStatus::Paused => "Paused".to_owned(),
                            PlaybackStatus::Stopped => "Stopped".to_owned(),
                        })),
                    );
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
        if let Some(position_ms) = seeked_position_ms {
            let msg = dbus::message::Message::signal(
                &dbus::Path::new("/org/mpris/MediaPlayer2").unwrap(),
                &dbus::strings::Interface::new("org.mpris.MediaPlayer2.Player").unwrap(),
                &dbus::strings::Member::new("Seeked").unwrap(),
            )
            // position should be in microseconds
            .append1(position_ms as i64 * 1000);
            conn.send(msg).unwrap();
        }
    }
}

fn get_device_id(
    sp_client: &AuthCodeSpotify,
    device_name: &str,
    only_active: bool,
) -> Option<String> {
    let device_result = sp_client.device();
    match device_result {
        Ok(devices) => devices.into_iter().find_map(|d| {
            if d.name == device_name && (d.is_active || !only_active) {
                info!("Found device: {}, active: {}", d.name, d.is_active);
                d.id
            } else {
                None
            }
        }),
        Err(err) => {
            error!("Get devices error: {}", err);
            None
        }
    }
}

fn uri_to_object_path(uri: String) -> dbus::Path<'static> {
    let mut path = String::with_capacity(uri.len() + 1);
    for element in uri.split(':') {
        path.push('/');
        path.push_str(element);
    }
    dbus::Path::new(path).unwrap()
}

fn insert_metadata(m: &mut HashMap<String, Variant<Box<dyn RefArg>>>, item: PlayableItem) {
    use rspotify::model::{
        Image,
        PlayableItem::{Episode, Track},
    };

    // some fields that only make sense or only exist for tracks
    struct TrackFields {
        artists: Vec<String>,
        popularity: u32,
        track_number: u32,
        disc_number: i32,
    }

    // a common denominator struct for FullEpisode and FullTrack
    struct TrackOrEpisode {
        id: Option<dbus::Path<'static>>,
        duration: std::time::Duration,
        images: Vec<Image>,
        name: String,
        album_name: String,
        album_artists: Vec<String>,
        external_urls: HashMap<String, String>,
        track_fields: Option<TrackFields>,
    }

    let item = match item {
        Track(t) => TrackOrEpisode {
            id: t.id.map(|t| uri_to_object_path(t.uri())),
            duration: t.duration,
            images: t.album.images,
            name: t.name,
            album_name: t.album.name,
            album_artists: t.album.artists.into_iter().map(|a| a.name).collect(),
            external_urls: t.external_urls,
            track_fields: Some(TrackFields {
                artists: t.artists.into_iter().map(|a| a.name).collect(),
                popularity: t.popularity,
                track_number: t.track_number,
                disc_number: t.disc_number,
            }),
        },
        Episode(e) => TrackOrEpisode {
            id: Some(uri_to_object_path(e.id.uri())),
            duration: e.duration,
            images: e.show.images,
            name: e.name,
            album_name: e.show.name,
            album_artists: vec![e.show.publisher],
            external_urls: e.external_urls,
            track_fields: None,
        },
    };

    m.insert(
        "mpris:trackid".to_string(),
        Variant(Box::new(item.id.unwrap_or_default())),
    );

    m.insert(
        "mpris:length".to_string(),
        Variant(Box::new(item.duration.as_micros() as i64)),
    );

    m.insert(
        "mpris:artUrl".to_string(),
        Variant(Box::new(
            item.images
                .into_iter()
                .max_by_key(|i| i.width.unwrap_or(0))
                .map(|i| i.url)
                .unwrap_or_default(),
        )),
    );

    m.insert("xesam:title".to_string(), Variant(Box::new(item.name)));

    m.insert(
        "xesam:album".to_string(),
        Variant(Box::new(item.album_name)),
    );

    m.insert(
        "xesam:albumArtist".to_string(),
        Variant(Box::new(item.album_artists)),
    );

    if let Some(track) = item.track_fields {
        m.insert("xesam:artist".to_string(), Variant(Box::new(track.artists)));

        m.insert(
            "xesam:autoRating".to_string(),
            Variant(Box::new(f64::from(track.popularity) / 100.0)),
        );

        m.insert(
            "xesam:trackNumber".to_string(),
            Variant(Box::new(track.track_number)),
        );

        m.insert(
            "xesam:discNumber".to_string(),
            Variant(Box::new(track.disc_number)),
        );
    }

    // to avoid cloning here, we take the relevant url directly from the HashMap
    let mut external_urls = item.external_urls;
    m.insert(
        "xesam:url".to_string(),
        Variant(Box::new(
            external_urls.remove("spotify").unwrap_or_default(),
        )),
    );
}

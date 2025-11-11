use crate::config::DBusType;
use chrono::{Duration, prelude::*};
use dbus::{
    MethodErr,
    arg::{RefArg, Variant},
    channel::{MatchingReceiver, Sender},
    message::{MatchRule, SignalArgs},
    nonblock::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged,
};
use dbus_crossroads::{Crossroads, IfaceToken};
use dbus_tokio::connection::{self, IOResourceError};
use futures::{
    Future,
    task::{Context, Poll},
};
use librespot_connect::{LoadContextOptions, LoadRequest, LoadRequestOptions, Spirc};
use librespot_core::{Session, SpotifyId, SpotifyUri};
use librespot_metadata::audio::AudioItem;
use librespot_playback::player::PlayerEvent;
use log::{debug, error, warn};
use std::convert::TryFrom;
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, RwLock},
};
use thiserror::Error;
use time::format_description::well_known::Iso8601;
use tokio::{
    runtime::Handle,
    sync::{
        Mutex,
        mpsc::{UnboundedReceiver, UnboundedSender},
    },
};

type DbusMap = HashMap<String, Variant<Box<dyn RefArg>>>;

const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const CONTROLS_PATH: &str = "/rs/spotifyd/Controls";

pub enum ControlMessage {
    SetSession(Arc<Spirc>, Session),
    DropSession,
    Shutdown,
}

pub(crate) struct DbusServer {
    dbus_future: Pin<Box<dyn Future<Output = Result<(), DbusError>>>>,
    control_tx: UnboundedSender<ControlMessage>,
}

impl DbusServer {
    pub fn new(event_rx: UnboundedReceiver<PlayerEvent>, dbus_type: DBusType) -> DbusServer {
        let (control_tx, control_rx) = tokio::sync::mpsc::unbounded_channel();
        let dbus_future = Box::pin(create_dbus_server(event_rx, control_rx, dbus_type));
        DbusServer {
            dbus_future,
            control_tx,
        }
    }

    pub fn set_session(&self, spirc: Arc<Spirc>, session: Session) -> Result<(), DbusError> {
        self.control_tx
            .send(ControlMessage::SetSession(spirc, session))
            .map_err(|_| DbusError::ControlChannelBroken)
    }

    pub fn drop_session(&self) -> Result<(), DbusError> {
        self.control_tx
            .send(ControlMessage::DropSession)
            .map_err(|_| DbusError::ControlChannelBroken)
    }

    /// Sends a shutdown signal and returns false, if the server was already shut down.
    pub fn shutdown(&self) -> bool {
        self.control_tx.send(ControlMessage::Shutdown).is_ok()
    }
}

#[derive(Debug, Error)]
pub(crate) enum DbusError {
    #[error("Failed to initialize D-Bus: {}", .0)]
    InitFailure(#[from] dbus::Error),
    #[error("The connection was terminated unexpectedly: {}", .0)]
    ConnectionFailure(#[from] IOResourceError),
    #[error("Unexpectedly lost control of dbus server")]
    ControlChannelBroken,
}

impl Future for DbusServer {
    type Output = Result<(), DbusError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.dbus_future.as_mut().poll(cx)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl PlaybackStatus {
    fn to_mpris(self) -> &'static str {
        match self {
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
            PlaybackStatus::Stopped => "Stopped",
        }
    }
}

#[derive(Debug)]
struct Position {
    last_position: Duration,
    last_update: DateTime<Local>,
}

impl Position {
    fn new() -> Self {
        Self {
            last_position: Duration::zero(),
            last_update: Local::now(),
        }
    }
    fn update_position(&mut self, new_position: Duration) {
        self.last_update = Local::now();
        self.last_position = new_position;
    }

    fn get_position(&self) -> Duration {
        (Local::now() - self.last_update) + self.last_position
    }
}

#[derive(Clone, Copy, Debug)]
enum RepeatState {
    None,
    Track,
    All,
}

impl RepeatState {
    fn to_mpris(self) -> &'static str {
        match self {
            RepeatState::None => "None",
            RepeatState::Track => "Track",
            RepeatState::All => "Playlist",
        }
    }

    fn repeat_track(self) -> bool {
        matches!(self, RepeatState::Track)
    }

    fn repeat_context(self) -> bool {
        !matches!(self, RepeatState::None)
    }
}

impl From<(bool, bool)> for RepeatState {
    fn from((context, track): (bool, bool)) -> Self {
        if context {
            if track {
                RepeatState::Track
            } else {
                RepeatState::All
            }
        } else {
            RepeatState::None
        }
    }
}

#[derive(Debug)]
struct CurrentStateInner {
    status: PlaybackStatus,
    position: Option<Position>,
    audio_item: Option<Box<AudioItem>>,
    volume: u16,
    shuffle: bool,
    repeat: RepeatState,
    play_request_id: Option<u64>,
}

fn insert_attr(map: &mut DbusMap, attr: impl ToString, value: impl RefArg + 'static) {
    map.insert(attr.to_string(), Variant(Box::new(value)));
}

impl CurrentStateInner {
    fn mpris_volume(&self) -> f64 {
        self.volume as f64 / u16::MAX as f64
    }

    fn get_position(&self) -> Option<Duration> {
        let position = self.position.as_ref()?;
        match self.status {
            PlaybackStatus::Playing => Some(position.get_position()),
            PlaybackStatus::Paused => Some(position.last_position),
            PlaybackStatus::Stopped => None,
        }
    }

    fn update_position(&mut self, position: Duration) {
        self.position
            .get_or_insert_with(Position::new)
            .update_position(position);
    }

    fn handle_event(&mut self, event: PlayerEvent) -> (DbusMap, bool) {
        let mut changed = DbusMap::new();
        let mut seeked = false;

        // note that get_play_request_id is None on PlayRequestIdChanged
        if Option::zip(self.play_request_id, event.get_play_request_id())
            .is_some_and(|(cur_id, event_id)| cur_id != event_id)
        {
            debug!("discarding event due to play_request_id mismatch");
            return (changed, seeked);
        }

        debug!("handling event {event:?}");
        match event {
            PlayerEvent::VolumeChanged { volume } => {
                self.volume = volume;
                insert_attr(&mut changed, "Volume", self.mpris_volume());
            }
            PlayerEvent::Stopped { .. } => {
                self.status = PlaybackStatus::Stopped;
                self.audio_item = None;
                insert_attr(
                    &mut changed,
                    "PlaybackStatus",
                    self.status.to_mpris().to_string(),
                );
                insert_attr(&mut changed, "Metadata", self.to_metadata());
            }
            PlayerEvent::Playing { position_ms, .. } => {
                if self.status != PlaybackStatus::Playing {
                    self.status = PlaybackStatus::Playing;
                    insert_attr(
                        &mut changed,
                        "PlaybackStatus",
                        self.status.to_mpris().to_string(),
                    );
                }
                self.update_position(Duration::milliseconds(position_ms as i64));
                seeked = true;
            }
            PlayerEvent::Paused { position_ms, .. } => {
                if self.status != PlaybackStatus::Paused {
                    self.status = PlaybackStatus::Paused;
                    insert_attr(
                        &mut changed,
                        "PlaybackStatus",
                        self.status.to_mpris().to_string(),
                    )
                }
                self.update_position(Duration::milliseconds(position_ms as i64));
                seeked = true;
            }
            PlayerEvent::TrackChanged { audio_item } => {
                self.audio_item = Some(audio_item);
                insert_attr(&mut changed, "Metadata", self.to_metadata());
            }
            PlayerEvent::PositionCorrection { position_ms, .. }
            | PlayerEvent::PositionChanged { position_ms, .. }
            | PlayerEvent::Seeked { position_ms, .. } => {
                self.update_position(Duration::milliseconds(position_ms as i64));
                seeked = true;
            }
            PlayerEvent::ShuffleChanged { shuffle } => {
                self.shuffle = shuffle;
                insert_attr(&mut changed, "Shuffle", self.shuffle);
            }
            PlayerEvent::RepeatChanged { context, track } => {
                self.repeat = (context, track).into();
                insert_attr(
                    &mut changed,
                    "LoopStatus",
                    self.repeat.to_mpris().to_string(),
                )
            }
            PlayerEvent::PlayRequestIdChanged { play_request_id } => {
                self.play_request_id = Some(play_request_id);
            }
            PlayerEvent::Preloading { .. }
            | PlayerEvent::Loading { .. }
            | PlayerEvent::TimeToPreloadNextTrack { .. }
            | PlayerEvent::EndOfTrack { .. }
            | PlayerEvent::Unavailable { .. }
            | PlayerEvent::AutoPlayChanged { .. }
            | PlayerEvent::FilterExplicitContentChanged { .. }
            | PlayerEvent::SessionConnected { .. }
            | PlayerEvent::SessionDisconnected { .. }
            | PlayerEvent::SessionClientChanged { .. } => (),
        }

        (changed, seeked)
    }

    fn to_metadata(&self) -> DbusMap {
        let mut m = HashMap::new();

        insert_attr(
            &mut m,
            "mpris:trackid",
            uri_to_object_path(
                self.audio_item
                    .as_deref()
                    .and_then(|item| item.track_id.to_uri().ok())
                    .as_deref(),
            ),
        );

        if let Some(audio_item) = self.audio_item.as_deref() {
            if let Some(length) =
                Duration::milliseconds(audio_item.duration_ms as i64).num_microseconds()
            {
                insert_attr(&mut m, "mpris:length", length);
            }

            if let Some(cover) = audio_item.covers.iter().max_by_key(|im| im.width) {
                insert_attr(&mut m, "mpris:artUrl", cover.url.clone());
            }

            insert_attr(&mut m, "xesam:title", audio_item.name.clone());

            use librespot_metadata::audio::UniqueFields::*;
            match &audio_item.unique_fields {
                Track {
                    artists,
                    album,
                    album_artists,
                    popularity,
                    number,
                    disc_number,
                } => {
                    insert_attr(
                        &mut m,
                        "xesam:artist",
                        artists
                            .iter()
                            .map(|artist| artist.name.clone())
                            .collect::<Vec<String>>(),
                    );
                    insert_attr(&mut m, "xesam:album", album.clone());
                    insert_attr(&mut m, "xesam:albumArtist", album_artists.clone());
                    insert_attr(&mut m, "xesam:autoRating", (*popularity as f64) / 100.0);
                    insert_attr(&mut m, "xesam:trackNumber", *number);
                    insert_attr(&mut m, "xesam:discNumber", *disc_number);
                }
                Episode {
                    description,
                    publish_time,
                    show_name,
                } => {
                    insert_attr(&mut m, "xesam:artist", vec![show_name.clone()]);
                    insert_attr(&mut m, "xesam:comment", vec![description.clone()]);
                    if let Ok(formatted_publish) = publish_time.format(&Iso8601::DEFAULT) {
                        insert_attr(&mut m, "xesam:contentCreated", formatted_publish);
                    }
                }
                Local { .. } => {
                    // Local files don't have additional metadata
                }
            }
        }

        m
    }
}

#[derive(Debug)]
struct CurrentState(RwLock<CurrentStateInner>);

#[derive(Clone, Copy, Debug, Error)]
#[error("internal state no longer available due to application error")]
struct StatePoisonError;

impl From<StatePoisonError> for MethodErr {
    fn from(value: StatePoisonError) -> Self {
        MethodErr::failed(&value)
    }
}

impl CurrentState {
    fn new(inner: CurrentStateInner) -> Self {
        Self(RwLock::new(inner))
    }

    fn read(&self) -> Result<std::sync::RwLockReadGuard<'_, CurrentStateInner>, StatePoisonError> {
        self.0.read().map_err(|_| StatePoisonError)
    }

    fn write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, CurrentStateInner>, StatePoisonError> {
        self.0.write().map_err(|_| StatePoisonError)
    }
}

async fn create_dbus_server(
    mut event_rx: UnboundedReceiver<PlayerEvent>,
    mut control_rx: UnboundedReceiver<ControlMessage>,
    dbus_type: DBusType,
) -> Result<(), DbusError> {
    let (resource, conn) = match dbus_type {
        DBusType::Session => connection::new_session_sync(),
        DBusType::System => connection::new_system_sync(),
    }?;
    let mut connection_task = tokio::spawn(async { Err::<(), _>(resource.await) });

    // this name will be used, once we can provide the mpris interface
    let mpris_name = format!(
        "org.mpris.MediaPlayer2.spotifyd.instance{}",
        std::process::id()
    );
    // this name will always be available to allow easy discovery of the controls interface
    let spotifyd_name = format!("rs.spotifyd.instance{}", std::process::id());

    conn.request_name(&spotifyd_name, false, true, true).await?;

    let mut cr = Crossroads::new();
    cr.set_async_support(Some((
        conn.clone(),
        Box::new(|x| {
            tokio::spawn(x);
        }),
    )));

    let current_state = Arc::new(CurrentState::new(CurrentStateInner {
        status: PlaybackStatus::Stopped,
        position: None,
        audio_item: None,
        volume: u16::MAX,
        shuffle: false,
        repeat: RepeatState::None,
        play_request_id: None,
    }));

    let (quit_tx, mut quit_rx) = tokio::sync::mpsc::unbounded_channel();

    let cr = Arc::new(Mutex::new(cr));

    let crossroads = cr.clone();
    conn.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |msg, conn| {
            tokio::task::block_in_place(|| {
                let mut cr = cr.blocking_lock();
                cr.handle_message(msg, conn).unwrap();
            });
            true
        }),
    );

    let mut spirc: Option<Arc<Spirc>> = None;
    let mut session: Option<Session> = None;

    struct ConnectionData {
        conn_id: String,
        seeked_fn: SeekedSignal,
    }
    let mut cur_conn: Option<ConnectionData> = None;

    loop {
        tokio::select! {
            _ = quit_rx.recv() => {
                break;
            }
            result = &mut connection_task => {
                result.expect("the dbus connection panicked")?;
                break;
            }
            event = event_rx.recv() => {
                let event = event.expect("event channel was unexpectedly closed");

                if let PlayerEvent::SessionConnected { connection_id, .. } = event {
                    let mut cr = crossroads.lock().await;
                    let seeked_fn = register_player_interface(
                        &mut cr,
                        spirc.clone().unwrap(),
                        session.clone().unwrap(),
                        current_state.clone(),
                        quit_tx.clone(),
                    );
                    if cur_conn.is_none() {
                        conn.request_name(&mpris_name, true, true, true).await?;
                    }
                    cur_conn = Some(ConnectionData { conn_id: connection_id, seeked_fn });
                } else if let PlayerEvent::SessionDisconnected { connection_id, .. } = event {
                    // if this message isn't outdated yet, we vanish from the bus
                    if cur_conn.as_ref().is_some_and(|d| d.conn_id == connection_id) {
                        let mut cr = crossroads.lock().await;
                        conn.release_name(&mpris_name).await?;
                        cr.remove::<()>(&MPRIS_PATH.into());
                        cur_conn = None;
                    }
                } else {
                    let (changed, seeked) = current_state
                        .write()
                        .expect("state has been poisoned")
                        .handle_event(event);

                    if seeked {
                        let position = current_state
                            .read()
                            .expect("state has been poisoned")
                            .get_position();
                        if let Some((ConnectionData { seeked_fn, .. }, position)) =
                            Option::zip(cur_conn.as_ref(), position)
                        {
                            let msg = seeked_fn(
                                &MPRIS_PATH.into(),
                                &(position.num_microseconds().unwrap_or_default(),),
                            );
                            conn.send(msg).unwrap();
                        }
                    }

                    if !changed.is_empty() {
                        let msg = PropertiesPropertiesChanged {
                            interface_name: "org.mpris.MediaPlayer2.Player".to_owned(),
                            changed_properties: changed,
                            invalidated_properties: Vec::new(),
                        };
                        conn.send(
                            msg.to_emit_message(&dbus::Path::new(MPRIS_PATH).unwrap()),
                        )
                        .unwrap();
                    }
                }

            }
            control = control_rx.recv() => {
                let control = control.expect("control channel was unexpectedly closed");
                match control {
                    ControlMessage::Shutdown => {
                        break;
                    },
                    ControlMessage::SetSession(new_spirc, new_session) => {
                        let mut cr = crossroads.lock().await;
                        register_controls_interface(&mut cr, new_spirc.clone());
                        spirc = Some(new_spirc);
                        session = Some(new_session);
                    }
                    ControlMessage::DropSession => {
                        let mut cr = crossroads.lock().await;
                        conn.release_name(&mpris_name).await?;
                        cr.remove::<()>(&MPRIS_PATH.into());
                        cr.remove::<()>(&CONTROLS_PATH.into());
                        spirc = None;
                        session = None;
                    }
                }
            }
        }
    }
    conn.release_name(&mpris_name).await?;
    conn.release_name(&spotifyd_name).await?;
    Ok(())
}

type SeekedSignal = Box<dyn Fn(&dbus::Path, &(i64,)) -> dbus::Message + Send + Sync + 'static>;

fn register_player_interface(
    cr: &mut Crossroads,
    spirc: Arc<Spirc>,
    session: Session,
    current_state: Arc<CurrentState>,
    quit_tx: tokio::sync::mpsc::UnboundedSender<()>,
) -> SeekedSignal {
    // The following methods and properties are part of the MediaPlayer2 interface.
    // https://specifications.freedesktop.org/mpris-spec/latest/Media_Player.html
    let media_player2_interface = cr.register("org.mpris.MediaPlayer2", move |b| {
        let mut quit_tx = Some(quit_tx);
        b.method("Raise", (), (), move |_, _, (): ()| {
            // noop
            Ok(())
        });
        b.method("Quit", (), (), move |_, _, (): ()| {
            quit_tx.take().unwrap().send(()).ok();
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

    let mut seeked_signal = None;

    let player_interface: IfaceToken<()> = cr.register("org.mpris.MediaPlayer2.Player", |b| {
        seeked_signal = Some(b.signal::<(i64,), _>("Seeked", ("Position",)).msg_fn());
        let local_spirc = spirc.clone();
        b.method("VolumeUp", (), (), move |_, _, (): ()| {
            local_spirc.volume_up().map_err(|e| MethodErr::failed(&e))
        })
        .deprecated();
        let local_spirc = spirc.clone();
        b.method("VolumeDown", (), (), move |_, _, (): ()| {
            local_spirc.volume_down().map_err(|e| MethodErr::failed(&e))
        })
        .deprecated();
        let local_spirc = spirc.clone();
        b.method("Next", (), (), move |_, _, (): ()| {
            local_spirc.next().map_err(|e| MethodErr::failed(&e))
        });
        let local_spirc = spirc.clone();
        b.method("Previous", (), (), move |_, _, (): ()| {
            local_spirc.prev().map_err(|e| MethodErr::failed(&e))
        });
        let local_spirc = spirc.clone();
        b.method("Pause", (), (), move |_, _, (): ()| {
            local_spirc.pause().map_err(|e| MethodErr::failed(&e))
        });
        let local_spirc = spirc.clone();
        b.method("PlayPause", (), (), move |_, _, (): ()| {
            warn!("PlayPause method called via mpris");
            local_spirc.play_pause().map_err(|e| MethodErr::failed(&e))
        });
        let local_spirc = spirc.clone();
        b.method("Play", (), (), move |_, _, (): ()| {
            warn!("Play method called via mpris");
            local_spirc.play().map_err(|e| MethodErr::failed(&e))
        });
        let local_spirc = spirc.clone();
        b.method("Stop", (), (), move |_, _, (): ()| {
            let pause_playback = false;
            local_spirc
                .disconnect(pause_playback)
                .map_err(|e| MethodErr::failed(&e))
        });

        let local_spirc = spirc.clone();
        let local_state = current_state.clone();
        b.method("Seek", ("offset",), (), move |_, _, (offset,): (i64,)| {
            let Some(position) = local_state.read()?.get_position() else {
                return Err(dbus::MethodErr::failed(
                    "cannot seek while playback is stopped",
                ));
            };
            let new_pos = position + Duration::microseconds(offset);
            let new_pos_ms = u32::try_from(new_pos.num_milliseconds()).map_err(|err| {
                dbus::MethodErr::invalid_arg(&format!("new position out of bounds: {err}"))
            })?;
            if let Err(err) = local_spirc.set_position_ms(new_pos_ms) {
                warn!("failed to seek by {offset}ms: {err}");
                return Err(dbus::MethodErr::failed(&err));
            }
            Ok(())
        });

        let local_spirc = spirc.clone();
        let local_state = current_state.clone();
        b.method(
            "SetPosition",
            ("track_id", "position"),
            (),
            move |_, _, (track_id, pos): (dbus::Path, i64)| {
                let Some((current_track_id, duration)) = local_state
                    .read()?
                    .audio_item
                    .as_ref()
                    .map(|item| (item.track_id.clone(), item.duration_ms))
                else {
                    return Err(dbus::MethodErr::failed(
                        "can set position while nothing is playing",
                    ));
                };
                let duration = Duration::milliseconds(duration.into());

                if !track_id.ends_with(&current_track_id.to_id().unwrap()) {
                    // as per mpris spec: ignore as stale
                    return Ok(());
                }
                let new_position = Duration::microseconds(pos);
                if new_position < Duration::zero() || new_position > duration {
                    // ignore as per spec
                    return Ok(());
                }
                if let Err(err) =
                    local_spirc.set_position_ms(new_position.num_milliseconds() as u32)
                {
                    return Err(dbus::MethodErr::failed(&err));
                }

                Ok(())
            },
        );

        let local_spirc = spirc.clone();
        let local_state = current_state.clone();
        b.method("OpenUri", ("uri",), (), move |_, _, (uri,): (String,)| {
            let spotify_uri = SpotifyUri::from_uri(&uri).map_err(|e| MethodErr::invalid_arg(&e))?;
            let id = SpotifyId::try_from(&spotify_uri).map_err(|e| MethodErr::invalid_arg(&e))?;
            let CurrentStateInner {
                shuffle, repeat, ..
            } = *local_state.read()?;

            let session = session.clone();
            let uri_for_context = spotify_uri.clone();

            let (playing_track_index, context_uri) = Handle::current()
                .block_on(async move {
                    use librespot_metadata::*;
                    Ok::<_, librespot_core::Error>(match &uri_for_context {
                        SpotifyUri::Track { .. } => {
                            let track = Track::get(&session, &uri_for_context).await?;
                            (track.number as u32, track.album.id.to_uri()?)
                        }
                        SpotifyUri::Album { .. }
                        | SpotifyUri::Artist { .. }
                        | SpotifyUri::Playlist { .. }
                        | SpotifyUri::Episode { .. }
                        | SpotifyUri::Show { .. } => (0, uri),
                        SpotifyUri::Local { .. } | SpotifyUri::Unknown { .. } => {
                            return Err(librespot_core::Error::unimplemented(
                                "this type of uri is not supported",
                            ));
                        }
                    })
                })
                .map_err(|e| MethodErr::failed(&e))?;

            warn!(
                "loading context_uri {context_uri} with playing_track_index {playing_track_index}"
            );

            local_spirc
                .load(LoadRequest::from_context_uri(
                    context_uri,
                    LoadRequestOptions {
                        start_playing: true,
                        seek_to: 0,
                        context_options: Some(LoadContextOptions::Options(
                            librespot_connect::Options {
                                shuffle,
                                repeat: repeat.repeat_context(),
                                repeat_track: repeat.repeat_track(),
                            },
                        )),
                        playing_track: Some(librespot_connect::PlayingTrack::Index(
                            playing_track_index,
                        )),
                    },
                ))
                .map_err(|e| MethodErr::failed(&e))
        });

        let local_state = current_state.clone();
        b.property("PlaybackStatus")
            .emits_changed_false()
            .get(move |_, _| {
                let playback_state = local_state.read()?.status;
                Ok(playback_state.to_mpris().to_string())
            });

        let local_spirc = spirc.clone();
        let local_state = current_state.clone();
        b.property("Shuffle")
            .emits_changed_false()
            .get(move |_, _| Ok(local_state.read()?.shuffle))
            .set(move |_, _, value| {
                local_spirc
                    .shuffle(value)
                    .map(|_| None)
                    .map_err(|err| dbus::MethodErr::failed(&err))
            });

        b.property("Rate").emits_changed_const().get(|_, _| Ok(1.0));
        b.property("MaximumRate")
            .emits_changed_const()
            .get(|_, _| Ok(1.0));
        b.property("MinimumRate")
            .emits_changed_const()
            .get(|_, _| Ok(1.0));

        let local_spirc = spirc.clone();
        let local_state = current_state.clone();
        b.property("Volume")
            .emits_changed_false()
            .get(move |_, _| Ok(local_state.read()?.mpris_volume()))
            .set(move |_, _, value| {
                if let Err(err) = local_spirc.set_volume((value * u16::MAX as f64) as u16) {
                    return Err(dbus::MethodErr::failed(&err));
                }
                Ok(None)
            });

        let local_spirc = spirc.clone();
        let local_state = current_state.clone();
        b.property("LoopStatus")
            .emits_changed_true()
            .get(move |_, _| {
                let repeat = local_state.read()?.repeat;
                Ok(repeat.to_mpris().to_string())
            })
            .set(move |_, _, value| {
                let repeat = match value.as_str() {
                    "None" => RepeatState::None,
                    "Playlist" => RepeatState::All,
                    "Track" => RepeatState::Track,
                    mode => {
                        return Err(dbus::MethodErr::failed(&format!(
                            "unsupported repeat mode: {mode}"
                        )));
                    }
                };

                local_spirc
                    .repeat(repeat.repeat_context())
                    .map_err(|e| MethodErr::failed(&e))?;
                local_spirc
                    .repeat_track(repeat.repeat_track())
                    .map_err(|e| MethodErr::failed(&e))?;

                Ok(None)
            });

        let local_state = current_state.clone();
        b.property("Position")
            .emits_changed_false()
            .get(move |_, _| {
                let Some(position) = local_state.read()?.get_position() else {
                    return Err(dbus::MethodErr::failed("no position available currently"));
                };

                Ok(position.num_microseconds().unwrap_or_default())
            });

        let local_state = current_state.clone();
        b.property("Metadata")
            .emits_changed_false()
            .get(move |_, _| Ok(local_state.read()?.to_metadata()));

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

    cr.insert(MPRIS_PATH, &[media_player2_interface, player_interface], ());

    seeked_signal.expect("player interface has not been registered")
}

fn register_controls_interface(cr: &mut Crossroads, spirc: Arc<Spirc>) {
    let spotifyd_ctrls_interface: IfaceToken<()> = cr.register("rs.spotifyd.Controls", |b| {
        let local_spirc = spirc.clone();
        b.method("VolumeUp", (), (), move |_, _, (): ()| {
            local_spirc.volume_up().map_err(|e| MethodErr::failed(&e))
        });
        let local_spirc = spirc.clone();
        b.method("VolumeDown", (), (), move |_, _, (): ()| {
            local_spirc.volume_down().map_err(|e| MethodErr::failed(&e))
        });

        let local_spirc = spirc.clone();
        b.method("TransferPlayback", (), (), move |_, _, (): ()| {
            local_spirc.activate().map_err(|e| MethodErr::failed(&e))
        });
    });

    cr.insert(CONTROLS_PATH, &[spotifyd_ctrls_interface], ());
}

fn uri_to_object_path(uri: Option<&str>) -> dbus::Path<'static> {
    let Some(uri) = uri else {
        return dbus::Path::new("/org/mpris/MediaPlayer2/TrackList/NoTrack").unwrap();
    };
    let mut path = String::with_capacity(uri.len() + 1);
    for element in uri.split(':') {
        path.push('/');
        path.push_str(element);
    }
    dbus::Path::new(path).unwrap()
}

#[cfg(feature = "dbus_mpris")]
use crate::config::{DBusType, MprisConfig};
#[cfg(feature = "dbus_mpris")]
use crate::dbus_mpris::DbusServer;
use crate::process::spawn_program_on_event;
use crate::utils::Backoff;
use color_eyre::eyre::{self, Context};
use futures::future::Either;
#[cfg(not(feature = "dbus_mpris"))]
use futures::future::Pending;
use futures::{
    self, Future, FutureExt, StreamExt,
    future::{self, Fuse, FusedFuture},
    stream::Peekable,
};
#[cfg(feature = "dbus_mpris")]
use librespot_connect::ClusterState;
use librespot_connect::{ConnectConfig, Spirc};
#[cfg(feature = "dbus_mpris")]
use librespot_core::SpotifyUri;
use librespot_core::{
    Error, SessionConfig, authentication::Credentials, cache::Cache, config::DeviceType,
    session::Session,
};
use librespot_discovery::Discovery;
#[cfg(feature = "dbus_mpris")]
use librespot_metadata::audio::AudioItem;
#[cfg(feature = "dbus_mpris")]
use librespot_playback::player::PlayerEvent;
use librespot_playback::{
    audio_backend::Sink,
    config::{AudioFormat, PlayerConfig},
    mixer::Mixer,
    player::Player,
};
#[cfg(feature = "dbus_mpris")]
use librespot_protocol::player::PlayerState;
use log::{error, info};
use std::pin::Pin;
use std::sync::Arc;
#[cfg(feature = "dbus_mpris")]
use tokio::sync::{mpsc::UnboundedSender, watch};

#[cfg(feature = "dbus_mpris")]
const SPECTATOR_CONNECTION_ID: &str = "spectator-remote";
#[cfg(feature = "dbus_mpris")]
const SPECTATOR_USER_NAME: &str = "spectator";
#[cfg(feature = "dbus_mpris")]
const SPECTATOR_PLAY_REQUEST_ID: u64 = 0;

#[cfg(not(feature = "dbus_mpris"))]
type DbusServer = Pending<()>;

pub(crate) enum CredentialsProvider {
    Discovery {
        stream: Peekable<Discovery>,
        last_credentials: Option<Credentials>,
    },
    CredentialsOnly(Credentials),
}

impl CredentialsProvider {
    async fn get_credentials(&mut self) -> Credentials {
        match self {
            CredentialsProvider::Discovery {
                stream,
                last_credentials,
            } => {
                let new_creds = match last_credentials.take() {
                    Some(creds) => stream.next().now_or_never().flatten().unwrap_or(creds),
                    None => stream.next().await.unwrap(),
                };
                *last_credentials = Some(new_creds.clone());
                new_creds
            }
            CredentialsProvider::CredentialsOnly(creds) => creds.clone(),
        }
    }

    // wait for an incoming connection if the underlying provider is a discovery stream
    async fn incoming_connection(&mut self) {
        match self {
            CredentialsProvider::Discovery { stream, .. } => {
                let peeked = Pin::new(stream).peek().await;
                if peeked.is_none() {
                    future::pending().await
                }
            }
            _ => future::pending().await,
        }
    }
}

pub(crate) struct MainLoop {
    pub(crate) session_config: SessionConfig,
    pub(crate) player_config: PlayerConfig,
    pub(crate) cache: Option<Cache>,
    pub(crate) mixer: Arc<dyn Mixer>,
    pub(crate) backend: fn(Option<String>, AudioFormat) -> Box<dyn Sink>,
    pub(crate) audio_device: Option<String>,
    pub(crate) audio_format: AudioFormat,
    pub(crate) disable_volume: bool,
    pub(crate) initial_volume: u16,
    pub(crate) shell: String,
    pub(crate) device_type: DeviceType,
    pub(crate) device_name: String,
    pub(crate) player_event_program: Option<String>,
    pub(crate) credentials_provider: CredentialsProvider,
    pub(crate) spectator: bool,
    #[cfg(feature = "dbus_mpris")]
    pub(crate) mpris_config: MprisConfig,
}

struct ConnectionInfo<SpircTask: Future<Output = ()>> {
    spirc: Spirc,
    #[cfg_attr(not(feature = "dbus_mpris"), expect(unused))]
    session: Session,
    player: Arc<Player>,
    spirc_task: SpircTask,
}

#[cfg(feature = "dbus_mpris")]
async fn fetch_audio_item(session: &Session, track_uri: &str) -> Option<Box<AudioItem>> {
    let uri = SpotifyUri::from_uri(track_uri).ok()?;
    AudioItem::get_file(session, uri).await.ok().map(Box::new)
}

/// Emits mpris session connect/disconnect on active-device transitions in the cluster snapshot.
#[cfg(feature = "dbus_mpris")]
fn sync_cluster_state(
    watch: &mut watch::Receiver<ClusterState>,
    has_active_remote: &mut bool,
    last_track_uri: &mut Option<String>,
    mpris_event_tx: &Option<UnboundedSender<PlayerEvent>>,
) {
    let now_active = watch.borrow_and_update().active_device_id.is_some();

    if now_active && !*has_active_remote {
        *has_active_remote = true;
        if let Some(tx) = mpris_event_tx {
            let _ = tx.send(PlayerEvent::SessionConnected {
                connection_id: SPECTATOR_CONNECTION_ID.to_string(),
                user_name: SPECTATOR_USER_NAME.to_string(),
            });
        }
    } else if !now_active && *has_active_remote {
        *has_active_remote = false;
        if let Some(tx) = mpris_event_tx {
            if let Some(uri) = last_track_uri.as_deref()
                && let Ok(track_id) = SpotifyUri::from_uri(uri)
            {
                let _ = tx.send(PlayerEvent::Stopped {
                    play_request_id: SPECTATOR_PLAY_REQUEST_ID,
                    track_id,
                });
            }
            let _ = tx.send(PlayerEvent::SessionDisconnected {
                connection_id: SPECTATOR_CONNECTION_ID.to_string(),
                user_name: SPECTATOR_USER_NAME.to_string(),
            });
        }
        *last_track_uri = None;
    }
}

/// Mirrors the current player snapshot's track/play state into mpris.
#[cfg(feature = "dbus_mpris")]
fn sync_player_state(
    watch: &mut watch::Receiver<Option<PlayerState>>,
    last_track_uri: &mut Option<String>,
    spectator_session: &Session,
    mpris_event_tx: &Option<UnboundedSender<PlayerEvent>>,
) {
    let Some(state) = watch.borrow_and_update().clone() else {
        return;
    };
    let Some(track_uri) = state
        .track
        .as_ref()
        .map(|t| t.uri.clone())
        .filter(|uri| !uri.is_empty())
    else {
        return;
    };
    let is_playing = state.is_playing;
    let is_paused = state.is_paused;
    let position_ms = extrapolated_position_ms(
        state.position_as_of_timestamp,
        state.timestamp,
        is_playing && !is_paused,
        chrono::Utc::now().timestamp_millis(),
    );

    if last_track_uri.as_deref() != Some(track_uri.as_str()) {
        let session = spectator_session.clone();
        let uri = track_uri.clone();
        let tx_clone = mpris_event_tx.clone();
        tokio::spawn(async move {
            if let Some(audio_item) = fetch_audio_item(&session, &uri).await
                && let Some(ref tx) = tx_clone
            {
                let _ = tx.send(PlayerEvent::TrackChanged { audio_item });
            }
        });
        *last_track_uri = Some(track_uri.clone());
    }

    if let Ok(track_id) = SpotifyUri::from_uri(&track_uri) {
        let event = if is_playing && !is_paused {
            PlayerEvent::Playing {
                play_request_id: SPECTATOR_PLAY_REQUEST_ID,
                track_id,
                position_ms,
            }
        } else {
            PlayerEvent::Paused {
                play_request_id: SPECTATOR_PLAY_REQUEST_ID,
                track_id,
                position_ms,
            }
        };
        if let Some(tx) = mpris_event_tx {
            let _ = tx.send(event);
        }
    }
}

/// `position_as_of_timestamp` is only accurate as of `timestamp`, so extrapolate forward.
#[cfg(feature = "dbus_mpris")]
fn extrapolated_position_ms(
    position_as_of_timestamp: i64,
    timestamp: i64,
    is_playing: bool,
    now_ms: i64,
) -> u32 {
    let elapsed_ms = if is_playing {
        (now_ms - timestamp).max(0)
    } else {
        0
    };
    (position_as_of_timestamp + elapsed_ms).max(0) as u32
}

#[cfg(all(test, feature = "dbus_mpris"))]
mod tests {
    use super::extrapolated_position_ms;

    #[test]
    fn playing_extrapolates_forward_from_stale_timestamp() {
        assert_eq!(extrapolated_position_ms(10_000, 1_000, true, 4_000), 13_000);
    }

    #[test]
    fn paused_does_not_extrapolate() {
        assert_eq!(
            extrapolated_position_ms(10_000, 1_000, false, 4_000),
            10_000
        );
    }

    #[test]
    fn negative_diff_clamps_to_zero_elapsed() {
        assert_eq!(extrapolated_position_ms(10_000, 4_000, true, 1_000), 10_000);
    }
}

impl MainLoop {
    async fn get_connection(
        &mut self,
    ) -> Result<ConnectionInfo<impl Future<Output = ()> + use<>>, Error> {
        let creds = self.credentials_provider.get_credentials().await;

        let mut connection_backoff = Backoff::default();
        loop {
            let session = Session::new(self.session_config.clone(), self.cache.clone());
            let player = {
                let audio_device = self.audio_device.clone();
                let audio_format = self.audio_format;
                let backend = self.backend;
                Player::new(
                    self.player_config.clone(),
                    session.clone(),
                    self.mixer.get_soft_volume(),
                    move || backend(audio_device, audio_format),
                )
            };

            // TODO: expose is_group
            match Spirc::new(
                ConnectConfig {
                    name: self.device_name.clone(),
                    device_type: self.device_type,
                    initial_volume: self.initial_volume,
                    disable_volume: self.disable_volume,
                    ..ConnectConfig::default()
                },
                session.clone(),
                creds.clone(),
                player.clone(),
                self.mixer.clone(),
            )
            .await
            {
                Ok((spirc, spirc_task)) => {
                    break Ok(ConnectionInfo {
                        spirc,
                        session,
                        player,
                        spirc_task,
                    });
                }
                Err(err) => {
                    let Ok(backoff) = connection_backoff.next_backoff() else {
                        break Err(err);
                    };
                    error!("connection to spotify failed: {err}");
                    info!(
                        "retrying connection in {} seconds (retry {}/{})",
                        backoff.as_secs(),
                        connection_backoff.retries(),
                        connection_backoff.max_retries()
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    pub(crate) async fn run(mut self) -> eyre::Result<()> {
        info!("spectator mode: {}", self.spectator);
        tokio::pin! {
            let ctrl_c = tokio::signal::ctrl_c();
            // we don't necessarily have a dbus server
            let dbus_server = Either::<DbusServer, _>::Right(future::pending());
        }

        #[cfg(feature = "dbus_mpris")]
        let mpris_event_tx = if self.mpris_config.use_mpris.unwrap_or(true) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            *dbus_server.as_mut() = Either::Left(DbusServer::new(
                rx,
                self.mpris_config.dbus_type.unwrap_or(DBusType::Session),
            ));
            Some(tx)
        } else {
            None
        };

        let mainloop_result: eyre::Result<()> = 'mainloop: loop {
            let connection = tokio::select!(
                _ = &mut ctrl_c => {
                    break 'mainloop Ok(());
                }
                connection = self.get_connection() => {
                    match connection {
                        Ok(connection) => connection,
                        Err(err) => break 'mainloop Err(err).wrap_err("failed to connect to spotify"),
                    }
                }
            );

            let spirc_task = connection.spirc_task;
            tokio::pin!(spirc_task);

            let shared_spirc = Arc::new(connection.spirc);
            #[cfg(feature = "dbus_mpris")]
            let spectator_session = connection.session.clone();

            #[cfg(feature = "dbus_mpris")]
            if let Either::Left(mut dbus_server) = Either::as_pin_mut(dbus_server.as_mut())
                && let Err(err) = dbus_server
                    .as_mut()
                    .set_session(shared_spirc.clone(), connection.session)
            {
                let _ = shared_spirc.shutdown();
                let _ = (&mut spirc_task).await;
                break 'mainloop Err(err).wrap_err("failed to configure dbus server");
            }

            let mut running_event_program = Box::pin(Fuse::terminated());

            let mut event_channel = connection.player.get_player_event_channel();

            let mut cluster_state_watch =
                self.spectator.then(|| shared_spirc.watch_cluster_state());
            let mut player_state_watch = self.spectator.then(|| shared_spirc.watch_player_state());
            let mut last_track_uri: Option<String> = None;
            let mut has_active_remote = false;

            // a fresh watch::Receiver doesn't treat its current value as a "change", so seed
            // once upfront in case a remote is already active
            #[cfg(feature = "dbus_mpris")]
            if let Some(watch) = cluster_state_watch.as_mut() {
                sync_cluster_state(
                    watch,
                    &mut has_active_remote,
                    &mut last_track_uri,
                    &mpris_event_tx,
                );
                if has_active_remote && let Some(watch) = player_state_watch.as_mut() {
                    sync_player_state(
                        watch,
                        &mut last_track_uri,
                        &spectator_session,
                        &mpris_event_tx,
                    );
                }
            }

            loop {
                tokio::select!(
                    // a new session has been started via the discovery stream
                    _ = self.credentials_provider.incoming_connection() => {
                        let _ = shared_spirc.shutdown();
                        let _ = (&mut spirc_task).await;
                        break;
                    }
                    // the program should shut down
                    _ = &mut ctrl_c => {
                        let _ = shared_spirc.shutdown();
                        let _ = (&mut spirc_task).await;
                        break 'mainloop Ok(());
                    }
                    // spirc was shut down by some external factor
                    _ = &mut spirc_task => {
                        break;
                    }
                    // dbus stopped unexpectedly
                    result = &mut dbus_server => {
                        #[cfg(feature = "dbus_mpris")]
                        {
                            let _ = shared_spirc.shutdown();
                            let _ = (&mut spirc_task).await;
                            *dbus_server.as_mut() = Either::Right(future::pending());
                            break 'mainloop result.wrap_err("DBus terminated unexpectedly");
                        }
                        #[cfg(not(feature = "dbus_mpris"))]
                        result // unused variable
                    }
                    // a cluster snapshot changed: track whether some remote device is active
                    cluster_changed = async {
                        match cluster_state_watch.as_mut() {
                            Some(watch) => watch.changed().await,
                            None => future::pending().await,
                        }
                    }, if self.spectator => {
                        #[cfg(feature = "dbus_mpris")]
                        if cluster_changed.is_ok() {
                            sync_cluster_state(
                                cluster_state_watch.as_mut().unwrap(),
                                &mut has_active_remote,
                                &mut last_track_uri,
                                &mpris_event_tx,
                            );
                            // player watch may not fire its own changed() here, so sync it too
                            if has_active_remote
                                && let Some(watch) = player_state_watch.as_mut()
                            {
                                sync_player_state(
                                    watch,
                                    &mut last_track_uri,
                                    &spectator_session,
                                    &mpris_event_tx,
                                );
                            }
                        }
                    }
                    // the remote player snapshot changed: mirror track/play state into mpris
                    player_changed = async {
                        match player_state_watch.as_mut() {
                            Some(watch) => watch.changed().await,
                            None => future::pending().await,
                        }
                    }, if self.spectator && has_active_remote => {
                        #[cfg(feature = "dbus_mpris")]
                        if player_changed.is_ok() {
                            sync_player_state(
                                player_state_watch.as_mut().unwrap(),
                                &mut last_track_uri,
                                &spectator_session,
                                &mpris_event_tx,
                            );
                        }
                    }
                    // a new player event is available and no program is running
                    event = event_channel.recv(), if running_event_program.is_terminated() && !self.spectator => {
                        let event = event.unwrap();
                        #[cfg(feature = "dbus_mpris")]
                        if let Some(ref tx) = mpris_event_tx {
                            tx.send(event.clone()).unwrap();
                        }
                        if let Some(ref cmd) = self.player_event_program {
                            match spawn_program_on_event(&self.shell, cmd, event) {
                                Ok(child) => running_event_program = Box::pin(child.wait().fuse()),
                                Err(e) => error!("{}", e),
                            }
                        }
                    }
                    // a running program has finished
                    result = &mut running_event_program, if !running_event_program.is_terminated() => {
                        match result {
                            // Exited without error...
                            Ok(_) => (),
                            // Exited with error...
                            Err(e) => error!("{}", e),
                        }
                    }
                )
            }
            #[cfg(feature = "dbus_mpris")]
            if let Either::Left(dbus_server) = Either::as_pin_mut(dbus_server.as_mut())
                && let Err(err) = dbus_server.drop_session()
            {
                break 'mainloop Err(err).wrap_err("failed to reconfigure DBus server");
            }
        };

        if let CredentialsProvider::Discovery { stream, .. } = self.credentials_provider {
            let _ = stream.into_inner().shutdown().await;
        }
        #[cfg(feature = "dbus_mpris")]
        if let Either::Left(dbus_server) = Either::as_pin_mut(dbus_server.as_mut())
            && dbus_server.shutdown()
            && let Err(err) = dbus_server.await
        {
            let err = Err(err).wrap_err("failed to shutdown DBus server");
            if mainloop_result.is_ok() {
                return err;
            } else {
                error!("additional error while shutting down: {err:?}");
            }
        }
        mainloop_result
    }
}

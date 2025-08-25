use crate::error::Error;
use librespot_metadata::audio::AudioItem;
use librespot_playback::player::PlayerEvent;
use log::info;
use std::{collections::HashMap, process::Stdio};
use tokio::{
    io::{self, AsyncWriteExt},
    process::{self, Command},
};

/// Spawns provided command in a subprocess using the provided shell.
fn spawn_program(shell: &str, cmd: &str, env: HashMap<&str, String>) -> Result<Child, Error> {
    info!(
        "Running {:?} using {:?} with environment variables {:?}",
        cmd, shell, env
    );
    let inner = Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .envs(env.iter())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::subprocess_with_err(shell, cmd, e))?;
    let child = Child::new(cmd.to_string(), inner, shell.to_string());
    Ok(child)
}

fn audio_item_to_env(audio_item: Box<AudioItem>, env: &mut HashMap<&str, String>) {
    env.insert(
        "TRACK_ID",
        audio_item.track_id.to_base62().unwrap_or_default(),
    );
    env.insert("TRACK_NAME", audio_item.name);
    env.insert("TRACK_DURATION", audio_item.duration_ms.to_string());
    if let Some(cover) = audio_item.covers.into_iter().max_by_key(|c| c.width) {
        env.insert("TRACK_COVER", cover.url);
    }
}

/// Spawns provided command in a subprocess using the provided shell.
/// Various environment variables are included in the subprocess's environment
/// depending on the `PlayerEvent` that was passed in.
pub(crate) fn spawn_program_on_event(
    shell: &str,
    cmd: &str,
    event: PlayerEvent,
) -> Result<Child, Error> {
    let mut env = HashMap::new();
    match event {
        PlayerEvent::TrackChanged { audio_item } => {
                        env.insert("PLAYER_EVENT", "change".to_string());
                        audio_item_to_env(audio_item, &mut env);
            }
        PlayerEvent::Playing {
                track_id,
                play_request_id,
                position_ms,
            } => {
                env.insert("PLAYER_EVENT", "start".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                env.insert("POSITION_MS", position_ms.to_string());
            }
        PlayerEvent::Stopped {
                track_id,
                play_request_id,
            } => {
                env.insert("PLAYER_EVENT", "stop".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            }
        PlayerEvent::Loading {
                track_id,
                play_request_id,
                position_ms,
            } => {
                env.insert("PLAYER_EVENT", "load".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                env.insert("POSITION_MS", position_ms.to_string());
            }
        PlayerEvent::Paused {
                track_id,
                play_request_id,
                position_ms,
            } => {
                env.insert("PLAYER_EVENT", "pause".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
                env.insert("POSITION_MS", position_ms.to_string());
            }
        PlayerEvent::TimeToPreloadNextTrack {
                track_id,
                play_request_id,
            } => {
                env.insert("PLAYER_EVENT", "preload".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            }
        PlayerEvent::EndOfTrack {
                track_id,
                play_request_id,
            } => {
                env.insert("PLAYER_EVENT", "endoftrack".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            }
        PlayerEvent::VolumeChanged { volume } => {
                env.insert("PLAYER_EVENT", "volumeset".to_string());
                env.insert("VOLUME", volume.to_string());
            }
        PlayerEvent::Unavailable {
                play_request_id,
                track_id,
            } => {
                env.insert("PLAYER_EVENT", "unavailable".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            }
        PlayerEvent::Preloading { track_id } => {
                env.insert("PLAYER_EVENT", "preloading".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
            }
        PlayerEvent::PositionCorrection {
                play_request_id,
                track_id,
                position_ms,
            } => {
                env.insert("PLAYER_EVENT", "positioncorrection".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("POSITION_MS", position_ms.to_string());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            }
        PlayerEvent::Seeked {
                play_request_id,
                track_id,
                position_ms,
            } => {
                env.insert("PLAYER_EVENT", "seeked".to_string());
                env.insert("TRACK_ID", track_id.to_base62().unwrap());
                env.insert("POSITION_MS", position_ms.to_string());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            }
        PlayerEvent::PlayRequestIdChanged { play_request_id } => {
                env.insert("PLAYER_EVENT", "playrequestid_changed".to_string());
                env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            }
        PlayerEvent::SessionConnected {
                connection_id,
                user_name,
            } => {
                env.insert("PLAYER_EVENT", "sessionconnected".to_string());
                env.insert("USERNAME", user_name);
                env.insert("CONNECTION_ID", connection_id);
            }
        PlayerEvent::SessionDisconnected {
                connection_id,
                user_name,
            } => {
                env.insert("PLAYER_EVENT", "sessiondisconnected".to_string());
                env.insert("USERNAME", user_name);
                env.insert("CONNECTION_ID", connection_id);
            }
        PlayerEvent::SessionClientChanged {
                client_id,
                client_name,
                client_brand_name,
                client_model_name,
            } => {
                env.insert("PLAYER_EVENT", "clientchanged".to_string());
                env.insert("CLIENT_ID", client_id);
                env.insert("CLIENT_NAME", client_name);
                env.insert("CLIENT_BRAND", client_brand_name);
                env.insert("CLIENT_MODEL", client_model_name);
            }
        PlayerEvent::ShuffleChanged { shuffle } => {
                env.insert("PLAYER_EVENT", "shuffle_changed".to_string());
                env.insert("SHUFFLE", shuffle.to_string());
            }
        PlayerEvent::RepeatChanged { context: _, track } => {
                env.insert("PLAYER_EVENT", "repeat_changed".to_string());
                let val = match track {
                    true => "all",
                    false => "none",
                }
                .to_string();
                env.insert("REPEAT", val);
            }
        PlayerEvent::AutoPlayChanged { auto_play } => {
                env.insert("PLAYER_EVENT", "autoplay_changed".to_string());
                env.insert("AUTOPLAY", auto_play.to_string());
            }
        PlayerEvent::FilterExplicitContentChanged { filter } => {
                env.insert("PLAYER_EVENT", "filterexplicit_changed".to_string());
                env.insert("FILTEREXPLICIT", filter.to_string());
            }
        PlayerEvent::PositionChanged { play_request_id: _, track_id: _, position_ms: _ } => todo!(),
    }
    spawn_program(shell, cmd, env)
}

/// Wraps `tokio::process::Child` so that when this `Child` exits:
/// * successfully: It writes the contents of it's stdout to the stdout of the
///   main process.
/// * unsuccesfully: It returns an error that includes the contents it's stderr
///   as well as information on the command that was run and the shell that
///   invoked it.
pub(crate) struct Child {
    cmd: String,
    child: process::Child,
    shell: String,
}

impl Child {
    pub(crate) fn new(cmd: String, child: process::Child, shell: String) -> Self {
        Self { cmd, child, shell }
    }

    pub(crate) async fn wait(self) -> Result<(), Error> {
        let Child { cmd, shell, child } = self;

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| Error::subprocess_with_err(&shell, &cmd, e))?;

        if output.status.success() {
            // If successful, write subprocess's stdout to main process's stdout...
            let mut stdout = io::stdout();

            stdout
                .write_all(&output.stdout)
                .await
                .map_err(|e| Error::subprocess_with_err(&shell, &cmd, e))?;

            stdout
                .flush()
                .await
                .map_err(|e| Error::subprocess_with_err(&shell, &cmd, e))?;

            Ok(())
        } else {
            // If unsuccessful, return an error that includes the contents of stderr...
            let err = match String::from_utf8(output.stderr) {
                Ok(stderr) => Error::subprocess_with_str(&shell, &cmd, &stderr),
                Err(_) => Error::subprocess(&shell, &cmd),
            };
            Err(err)
        }
    }
}

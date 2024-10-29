use crate::error::Error;
use librespot_playback::player::PlayerEvent;
use log::info;
use std::{collections::HashMap, process::Stdio};
use tokio::{
    io::{self, AsyncWriteExt},
    process::{self, Command},
};

/// Blocks while provided command is run in a subprocess using the provided
/// shell. If successful, returns the contents of the subprocess's `stdout` as a
/// `String`.
pub(crate) fn run_program(shell: &str, cmd: &str) -> Result<String, Error> {
    info!("Running {:?} using {:?}", cmd, shell);
    let output = std::process::Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| Error::subprocess_with_err(shell, cmd, e))?;
    if !output.status.success() {
        let s = std::str::from_utf8(&output.stderr).map_err(|_| Error::subprocess(shell, cmd))?;
        return Err(Error::subprocess_with_str(shell, cmd, s));
    }
    let s = String::from_utf8(output.stdout).map_err(|_| Error::subprocess(shell, cmd))?;
    Ok(s)
}

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
        PlayerEvent::PlayRequestIdChanged { play_request_id } => {
            env.insert("PLAYER_EVENT", "play_request_id_changed".to_string());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
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
        PlayerEvent::Preloading { track_id } => {
            env.insert("PLAYER_EVENT", "preloading".to_string());
            env.insert("TRACK_ID", track_id.to_base62().unwrap());
        }
        PlayerEvent::Playing {
            track_id,
            play_request_id,
            position_ms,
        } => {
            env.insert("PLAYER_EVENT", "play".to_string());
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
        PlayerEvent::Unavailable {
            play_request_id,
            track_id,
        } => {
            env.insert("PLAYER_EVENT", "unavailable".to_string());
            env.insert("TRACK_ID", track_id.to_base62().unwrap());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
        }
        PlayerEvent::VolumeChanged { volume } => {
            env.insert("PLAYER_EVENT", "volume_changed".to_string());
            env.insert("VOLUME", volume.to_string());
        }
        PlayerEvent::PositionCorrection {
            play_request_id,
            track_id,
            position_ms,
        } => {
            env.insert("PLAYER_EVENT", "position_correction".to_string());
            env.insert("TRACK_ID", track_id.to_base62().unwrap());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            env.insert("POSITION_MS", position_ms.to_string());
        }
        PlayerEvent::Seeked {
            play_request_id,
            track_id,
            position_ms,
        } => {
            env.insert("PLAYER_EVENT", "seeked".to_string());
            env.insert("TRACK_ID", track_id.to_base62().unwrap());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            env.insert("POSITION_MS", position_ms.to_string());
        }
        PlayerEvent::TrackChanged { audio_item } => {
            // TODO: Do we want other audio_item fields?
            env.insert("PLAYER_EVENT", "track_changed".to_string());
            env.insert("TRACK_ID", audio_item.track_id.to_base62().unwrap());
        }
        PlayerEvent::SessionConnected {
            connection_id,
            user_name,
        } => {
            env.insert("PLAYER_EVENT", "session_connected".to_string());
            env.insert("CONNECTION_ID", connection_id);
            env.insert("USER_NAME", user_name);
        }
        PlayerEvent::SessionDisconnected {
            connection_id,
            user_name,
        } => {
            env.insert("PLAYER_EVENT", "session_disconnected".to_string());
            env.insert("CONNECTION_ID", connection_id);
            env.insert("USER_NAME", user_name);
        }
        PlayerEvent::SessionClientChanged {
            client_id,
            client_name,
            client_brand_name,
            client_model_name,
        } => {
            env.insert("PLAYER_EVENT", "session_client_changed".to_string());
            env.insert("CLIENT_ID", client_id);
            env.insert("CLIENT_NAME", client_name);
            env.insert("CLIENT_BRAND_NAME", client_brand_name);
            env.insert("CLIENT_MODEL_NAME", client_model_name);
        }
        PlayerEvent::ShuffleChanged { shuffle } => {
            env.insert("PLAYER_EVENT", "shuffle_changed".to_string());
            env.insert("SHUFFLE", shuffle.to_string());
        }
        PlayerEvent::RepeatChanged { repeat } => {
            env.insert("PLAYER_EVENT", "repeat_changed".to_string());
            env.insert("REPEAT", repeat.to_string());
        }
        PlayerEvent::AutoPlayChanged { auto_play } => {
            env.insert("PLAYER_EVENT", "auto_play_changed".to_string());
            env.insert("AUTO_PLAY", auto_play.to_string());
        }
        PlayerEvent::FilterExplicitContentChanged { filter } => {
            env.insert(
                "PLAYER_EVENT",
                "filter_explicit_content_changed".to_string(),
            );
            env.insert("FILTER", filter.to_string());
        }
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

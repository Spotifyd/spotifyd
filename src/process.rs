use crate::error::Error;
use futures::Future;
use librespot_playback::player::PlayerEvent;
use log::info;
use std::{
    collections::HashMap,
    io::{self, Write},
    pin::Pin,
    process::{Output, Stdio},
    task::{Context, Poll},
};
use tokio::process::Command;

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
        PlayerEvent::Changed {
            old_track_id,
            new_track_id,
        } => {
            env.insert("OLD_TRACK_ID", old_track_id.to_base62());
            env.insert("PLAYER_EVENT", "change".to_string());
            env.insert("TRACK_ID", new_track_id.to_base62());
        }
        PlayerEvent::Started {
            track_id,
            play_request_id,
            position_ms,
        } => {
            env.insert("PLAYER_EVENT", "start".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            env.insert("POSITION_MS", position_ms.to_string());
        }
        PlayerEvent::Stopped {
            track_id,
            play_request_id,
        } => {
            env.insert("PLAYER_EVENT", "stop".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
        }
        PlayerEvent::Loading {
            track_id,
            play_request_id,
            position_ms,
        } => {
            env.insert("PLAYER_EVENT", "load".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            env.insert("POSITION_MS", position_ms.to_string());
        }
        PlayerEvent::Playing {
            track_id,
            play_request_id,
            position_ms,
            duration_ms,
        } => {
            env.insert("PLAYER_EVENT", "play".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            env.insert("POSITION_MS", position_ms.to_string());
            env.insert("DURATION_MS", duration_ms.to_string());
        }
        PlayerEvent::Paused {
            track_id,
            play_request_id,
            position_ms,
            duration_ms,
        } => {
            env.insert("PLAYER_EVENT", "pause".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
            env.insert("POSITION_MS", position_ms.to_string());
            env.insert("DURATION_MS", duration_ms.to_string());
        }
        PlayerEvent::TimeToPreloadNextTrack {
            track_id,
            play_request_id,
        } => {
            env.insert("PLAYER_EVENT", "preload".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
        }
        PlayerEvent::EndOfTrack {
            track_id,
            play_request_id,
        } => {
            env.insert("PLAYER_EVENT", "endoftrack".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
        }
        PlayerEvent::VolumeSet { volume } => {
            env.insert("PLAYER_EVENT", "volumeset".to_string());
            env.insert("VOLUME", volume.to_string());
        }
        PlayerEvent::Unavailable {
            play_request_id,
            track_id,
        } => {
            env.insert("PLAYER_EVENT", "unavailable".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
            env.insert("PLAY_REQUEST_ID", play_request_id.to_string());
        }
        PlayerEvent::Preloading { track_id } => {
            env.insert("PLAYER_EVENT", "preloading".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
        }
    }
    spawn_program(shell, cmd, env)
}

/// Wraps a process into a Future that executes something after the process has
/// exited:
/// * successfully: It writes the contents of it's stdout to the stdout of the
///   main process.
/// * unsuccesfully: It returns an error that includes the contents it's stderr
///   as well as information on the command that was run and the shell that
///   invoked it.
pub(crate) struct Child {
    cmd: String,
    future: Pin<Box<dyn Future<Output = io::Result<Output>>>>,
    shell: String,
}

impl Child {
    pub(crate) fn new(cmd: String, child: tokio::process::Child, shell: String) -> Self {
        Self {
            cmd,
            future: Box::pin(child.wait_with_output()),
            shell,
        }
    }
}

impl Future for Child {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Child>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready(result) = self.future.as_mut().poll(cx) {
            let output = match result {
                Ok(output) => output,
                Err(err) => {
                    return Poll::Ready(Err(Error::subprocess_with_err(
                        &self.shell,
                        &self.cmd,
                        err,
                    )));
                }
            };

            if output.status.success() {
                // If successful, write subprocess's stdout to main process's stdout...
                let stdout = io::stdout();
                let mut writer = stdout.lock();

                writer
                    .write_all(&output.stdout)
                    .map_err(|e| Error::subprocess_with_err(&self.shell, &self.cmd, e))?;

                writer
                    .flush()
                    .map_err(|e| Error::subprocess_with_err(&self.shell, &self.cmd, e))?;

                Poll::Ready(Ok(()))
            } else {
                // If unsuccessful, return an error that includes the contents of stderr...
                let stderr = String::from_utf8(output.stderr);
                match stderr {
                    Ok(stderr) => Err(Error::subprocess_with_str(&self.shell, &self.cmd, &stderr)),
                    Err(_) => Err(Error::subprocess(&self.shell, &self.cmd)),
                }?
            }
        } else {
            Poll::Pending
        }
    }
}

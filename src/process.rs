use librespot::playback::player::PlayerEvent;
use log::info;
use std::io::Read;
use std::{
    collections::HashMap,
    process::{Command, Stdio},
};

use crate::error::Error;

/// Blocks while provided bash command is run in a subprocess.
/// If successful, returns the contents of the subprocess's `stdout` as a `String`.
pub(crate) fn run_program(cmd: &str) -> Result<String, Error> {
    info!("Running {:?}", cmd);
    let output = Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| Error::subprocess_with_err(cmd, e))?;
    if !output.status.success() {
        let s = std::str::from_utf8(&output.stderr).map_err(|_| Error::subprocess(cmd))?;
        return Err(Error::subprocess_with_str(cmd, s));
    }
    let s = String::from_utf8(output.stdout).map_err(|_| Error::subprocess(cmd))?;
    Ok(s)
}

/// Spawns provided bash command in a subprocess, which does **not**
/// inheret it's parent's stdin, stdout, and stderr. If successful, returns a handle
/// to the subprocess, which in turn contains handles to subprocess's distinct stdin,
/// stdout, and stderr.
fn spawn_program(cmd: &str, env: HashMap<&str, String>) -> Result<Child, Error> {
    info!("Running {:?} with environment variables {:?}", cmd, env);
    let child = Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .envs(env.iter())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::subprocess_with_err(cmd, e))?;
    Ok(Child::new(cmd.to_string(), child))
}

/// Spawns provided bash command in a subprocess, providing it various
/// environment variables depending on the `PlayerEvent` that was passed in.
/// If successful, returns a handle to the subprocess.
pub(crate) fn spawn_program_on_event(cmd: &str, event: PlayerEvent) -> Result<Child, Error> {
    let mut env = HashMap::new();
    match event {
        PlayerEvent::Changed {
            old_track_id,
            new_track_id,
        } => {
            env.insert("PLAYER_EVENT", "change".to_string());
            env.insert("OLD_TRACK_ID", old_track_id.to_base62());
            env.insert("TRACK_ID", new_track_id.to_base62());
        }
        PlayerEvent::Started { track_id } => {
            env.insert("PLAYER_EVENT", "start".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
        }
        PlayerEvent::Stopped { track_id } => {
            env.insert("PLAYER_EVENT", "stop".to_string());
            env.insert("TRACK_ID", track_id.to_base62());
        }
    }
    spawn_program(cmd, env)
}

/// Same as a `std::process::Child` except this `Child`'s `wait` and `try_wait`
/// methods return an error if the subprocess exited unsuccesfully. This error
/// contains a) information on the original command that was run and b) the contents
/// of the subprocess's stderr, thereby enabling us to log that something bad happened
/// with this particular command.
#[derive(Debug)]
pub(crate) struct Child {
    cmd: String,
    inner: std::process::Child,
}

impl Child {
    pub(crate) fn new(cmd: String, child: std::process::Child) -> Self {
        Self { cmd, inner: child }
    }

    #[allow(unused)]
    pub(crate) fn wait(&mut self) -> Result<(), Error> {
        match self.inner.wait() {
            Ok(status) => {
                if !status.success() {
                    let mut buf = String::new();
                    match self.inner.stderr.as_mut().unwrap().read_to_string(&mut buf) {
                        Ok(_nread) => Err(Error::subprocess_with_str(&self.cmd, &buf)),
                        Err(e) => Err(Error::subprocess_with_err(&self.cmd, e)),
                    }
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(Error::subprocess_with_err(&self.cmd, e)),
        }
    }

    #[allow(unused)]
    pub(crate) fn try_wait(&mut self) -> Result<Option<()>, Error> {
        match self.inner.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let mut buf = String::new();
                    match self.inner.stderr.as_mut().unwrap().read_to_string(&mut buf) {
                        Ok(_nread) => Err(Error::subprocess_with_str(&self.cmd, &buf)),
                        Err(e) => Err(Error::subprocess_with_err(&self.cmd, e)),
                    }
                } else {
                    Ok(Some(()))
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(Error::subprocess_with_err(&self.cmd, e)),
        }
    }
}

impl std::ops::Deref for Child {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for Child {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl From<Child> for std::process::Child {
    fn from(child: Child) -> Self {
        child.inner
    }
}

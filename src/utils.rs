use log::trace;
use std::{env, time::Duration};

#[cfg(any(
    target_os = "freebsd",
    target_os = "linux",
    target_os = "openbsd",
    target_os = "android"
))]
fn get_shell_ffi() -> Option<String> {
    use libc::{geteuid, getpwuid_r};
    use std::{ffi::CStr, mem, ptr};

    trace!("Retrieving user shell through libc calls");

    let mut result = ptr::null_mut();
    unsafe {
        let amt: usize = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
            n if n < 0 => 512,
            n => n as usize,
        };
        let mut buf = Vec::with_capacity(amt);
        let mut passwd: libc::passwd = mem::zeroed();

        match getpwuid_r(
            geteuid(),
            &mut passwd,
            buf.as_mut_ptr(),
            buf.capacity() as libc::size_t,
            &mut result,
        ) {
            0 if !result.is_null() => {
                let ptr = passwd.pw_shell as *const _;
                let shell = CStr::from_ptr(ptr)
                    .to_str()
                    .expect("Failed to retrieve shell")
                    .to_owned();
                Some(shell)
            }
            _ => None,
        }
    }
}

// If the SHELL environment variable is not set and on we're on macOS,
// query the Directory Service command line utility (dscl) for the user's shell,
// as macOS does not use the /etc/passwd file
#[cfg(target_os = "macos")]
fn get_shell_ffi() -> Option<String> {
    use std::process::Command;

    trace!("Retrieving user shell through Directory Discovery Services");

    let username = whoami::username();
    let output = Command::new("dscl")
        .args([".", "-read", &format!("/Users/{}", username), "UserShell"])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = std::str::from_utf8(&output.stdout).ok()?;
        // The output of the dscl command should be:
        // "UserShell: /path/to/shell"
        if stdout.starts_with("UserShell: ") {
            let shell = stdout.split_whitespace().nth(1)?;
            return Some(shell.to_string());
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn get_shell_ffi() -> Option<String> {
    Some(String::from("cmd"))
}

pub(crate) fn get_shell() -> Option<String> {
    let shell = env::var("SHELL").ok().or_else(get_shell_ffi);
    trace!("Found user shell: {:?}", &shell);

    shell
}

/// Implements basic exponential backoff.
/// The used base is 2.
pub(crate) struct Backoff {
    retry_counter: u8,
    max_retries: u8,
    initial_backoff: Duration,
}

impl Backoff {
    pub(crate) fn new(max_retries: u8, initial_backoff: Duration) -> Self {
        Self {
            retry_counter: 0,
            max_retries,
            initial_backoff,
        }
    }

    /// Get the next backoff duration.
    ///
    /// Increases the retry counter and returns the next required backoff duration,
    /// if available.
    pub(crate) fn next_backoff(&mut self) -> Result<Duration, ()> {
        if self.retry_counter >= self.max_retries {
            return Err(());
        }
        let backoff_duration = self.initial_backoff * 2u32.pow(self.retry_counter as u32);
        self.retry_counter += 1;
        Ok(backoff_duration)
    }

    pub(crate) fn retries(&self) -> u8 {
        self.retry_counter
    }

    pub(crate) fn max_retries(&self) -> u8 {
        self.max_retries
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(4, Duration::from_secs(5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_logger() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn test_envvar_discovery() {
        init_logger();

        // SAFETY: this is part of the single-threaded tests
        unsafe {
            env::set_var("SHELL", "fantasy_shell");
        }

        let shell = get_shell().unwrap();
        assert_eq!(shell, "fantasy_shell");
    }

    #[test]
    fn test_ffi_discovery() {
        init_logger();

        let shell = get_shell_ffi();
        assert!(shell.is_some());
    }
}

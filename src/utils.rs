use whoami;

use log::{warn, trace};
use std::env;

pub(crate) fn get_shell() -> Option<String> {
    // First look for the user's preferred shell using the SHELL environment
    // variable...
    if let Ok(shell) = env::var("SHELL") {
        trace!("Found shell {:?} using SHELL environment variable.", shell);
        return Some(shell);
    }

    // If the SHELL environment variable is not set and we're on linux or one of the
    // BSDs, try to obtain the default shell from `/etc/passwd`...
    #[cfg(not(target_os = "macos"))]
    {
        use std::{
            fs::File,
            io::{self, BufRead},
        };

        let username = whoami::username();

        let file = File::open("/etc/passwd").ok()?;
        let reader = io::BufReader::new(file);
        // Each line of `/etc/passwd` describes a single user and contains seven
        // colon-separated fields: "name:password:UID:GID:GECOS:directory:shell"
        for line in reader.lines() {
            let line = line.ok()?;
            let mut iter = line.split(':');
            if let Some(user) = iter.nth(0) {
                if user == username {
                    let shell = iter.nth(5)?;
                    trace!("Found shell {:?} using /etc/passwd.", shell);
                    return Some(shell.into());
                }
            }
        }
    }

    // If the SHELL environment variable is not set and on we're on macOS,
    // query the Directory Service command line utility (dscl) for the user's shell,
    // as macOS does not use the /etc/passwd file...
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let username = whoami::username();
        let output = Command::new("dscl")
            .args(&[".", "-read", &format!("/Users/{}", username), "UserShell"])
            .output()
            .ok()?;
        if output.status.success() {
            let stdout = std::str::from_utf8(&output.stdout).ok()?;
            // The output of this dscl command should be:
            // "UserShell: /path/to/shell"
            if stdout.starts_with("UserShell: ") {
                let shell = stdout.split_whitespace().nth(1)?;
                trace!("Found shell {:?} using dscl command.", shell);
                return Some(shell.to_string());
            }
        }
    }
    None
}

pub fn contains_whitespace(s: &str) -> bool {
    let found_space = s.find(|c: char| c.is_whitespace()) != None;
    if found_space {
        warn!("device name contains whitespace. Set to default!");
    }

    found_space
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_shell() {
        env::set_var("RUST_LOG", "spotifyd=trace");

        env_logger::init();

        let _ = get_shell().unwrap();

        if env::var("SHELL").is_ok() {
            env::remove_var("SHELL");
            let _ = get_shell().unwrap();
        }
    }

    #[test]
    fn test_contains_whitespace() {
        assert!(contains_whitespace("hi there"));
        assert!(contains_whitespace(" hi there "));
        assert!(!contains_whitespace("hithere"));
    }
}

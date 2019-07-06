use std::cell::RefCell;
use std::env;
use std::ffi::CStr;

extern "C" {
    fn getlogin_r(buf: *mut libc::c_char, size: libc::size_t) -> libc::c_int;
}

thread_local! {
    static BUF_HOSTNAME: RefCell<[libc::c_char; 255]> = RefCell::new([0; 255]);
    static BUF_USERNAME: RefCell<[libc::c_char; 255]> = RefCell::new([0; 255]);
}

pub(crate) fn get_hostname() -> Option<String> {
    BUF_HOSTNAME.with(|refcell| {
        let mut buf = refcell.borrow_mut();
        let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as _, buf.len() as _) };
        if ret != 0 {
            return None;
        }
        let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
        let hostname = cstr.to_string_lossy().to_string();
        log::trace!("Found hostname {:?} using gethostname.", hostname);
        Some(hostname)
    })
}

pub(crate) fn get_shell() -> Option<String> {
    // First look for the user's preferred shell using the SHELL environment variable...
    if let Ok(shell) = env::var("SHELL") {
        log::trace!("Found shell {:?} using SHELL environment variable.", shell);
        return Some(shell);
    }

    // If the SHELL environment variable is not set and we're on linux or one of the BSDs,
    // try to obtain the default shell from `/etc/passwd`...
    #[cfg(not(target_os = "macos"))]
    {
        use std::fs::File;
        use std::io::{self, BufRead};

        let username = get_username()?;

        let file = File::open("/etc/passwd").ok()?;
        let reader = io::BufReader::new(file);
        // Each line of `/etc/passwd` describes a single user and contains seven colon-separated fields:
        // "name:password:UID:GID:GECOS:directory:shell"
        for line in reader.lines() {
            let line = line.ok()?;
            let mut iter = line.split(":");
            if let Some(user) = iter.nth(0) {
                if user == username {
                    let shell = iter.nth(5)?;
                    log::trace!("Found shell {:?} using /etc/passwd.", shell);
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

        let username = get_username()?;
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
                log::trace!("Found shell {:?} using dscl command.", shell);
                return Some(shell.to_string());
            }
        }
    }
    None
}

fn get_username() -> Option<String> {
    BUF_USERNAME.with(|refcell| {
        let mut buf = refcell.borrow_mut();
        let ret = unsafe { getlogin_r(buf.as_mut_ptr() as _, buf.len() as _) };
        if ret != 0 {
            return None;
        }
        let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
        let username = cstr.to_string_lossy().to_string();
        log::trace!("Found username: {:?} using getlogin_r", username);
        Some(username)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        env::set_var("RUST_LOG", "spotifyd=trace");

        env_logger::init();

        let _ = get_hostname().unwrap();

        let _ = get_shell().unwrap();

        if env::var("SHELL").is_ok() {
            env::remove_var("SHELL");
            let _ = get_shell().unwrap();
        }
    }
}

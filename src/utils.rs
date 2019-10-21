use log::trace;

use std::env;

pub(crate) fn get_shell() -> Option<String> {
    let shell = env::var("SHELL").ok().or_else(|| get_shell_ffi());
    trace!("Found user shell: {:?}", &shell);

    shell
}

#[cfg(target_os = "macos")]
fn get_shell_ffi() -> Option<String> {
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
            log::trace!("Found shell {:?} using dscl command.", shell);
            return Some(shell.to_string());
        }
    }

    None
}

#[cfg(target_os = "linux")]
fn get_shell_ffi() -> Option<String> {
    use libc::{getpwuid_r, geteuid};
    
    use std::ffi::CStr;
    use std::mem;
    use std::ptr;

    let mut result = ptr::null_mut();
    
    unsafe {
        let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
            n if n < 0 => 512 as usize,
            n => n as usize,
        };
        let mut buf = Vec::with_capacity(amt);
        let mut passwd: libc::passwd = mem::zeroed();

        match getpwuid_r(geteuid(), &mut passwd, buf.as_mut_ptr(),
                                buf.capacity() as libc::size_t,
                                &mut result) {
            0 if !result.is_null() => {
                let ptr = passwd.pw_shell as *const _;
                let username = CStr::from_ptr(ptr).to_str().unwrap().to_owned();
                Some(username)
            },
            _ => None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        env::set_var("RUST_LOG", "spotifyd=trace");

        env_logger::init();

        let _ = get_shell().unwrap();

        if env::var("SHELL").is_ok() {
            env::remove_var("SHELL");
            let _ = get_shell().unwrap();
        }
    }
}

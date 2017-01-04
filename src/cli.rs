use librespot::audio_backend::BACKENDS;
use getopts::Options;

pub fn usage(program: &str, opts: &Options) -> String {
    let brief = format!("Usage: {} [options]", program);
    format!("{}", opts.usage(&brief))
}

pub fn print_backends() {
    println!("Available backends:");
    for &(name, _) in BACKENDS {
        println!("- {}", name);
    }
}

pub fn command_line_argument_options() -> Options {
    let mut opts = Options::new();
    opts.optopt("c", "config", "Path to a config file.", "CONFIG");
    opts.optopt("u", "username", "Spotify user name.", "USERNAME");
    opts.optopt("p", "password", "Spotify password.", "PASSWORD");
    opts.optopt("", "pid", "Path to PID file.", "PID-FILE");
    if cfg!(feature = "facebook") {
        opts.optflag("", "facebook", "Login with a Facebook account");
    }
    opts.optflag("v", "verbose", "Add debug information to log.");
    opts.optflag("", "no-daemon", "Don't detach from console.");
    opts.optflag("", "backends", "Available audio backends.");
    opts.optflag("h", "help", "Print this help text.");
    opts
}

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
    opts.optopt("", "device", "Audio device, given by aplay -L.", "DEVICE");
    opts.optopt("", "bitrate", "Any of 96, 160, and 320.", "DEVICE");
    opts.optopt("", "pid", "Path to PID file.", "PID-FILE");
    opts.optopt("", "device_name", "Name of this Spotify device.", "DEVICE");
    opts.optopt("", "backend", "Audio backend.", "BACKEND");
    opts.optopt("", "cache_path", "Path to cache location.", "PATH");
    opts.optopt("", "onstart", "Command to run on song start.", "COMMAND");
    opts.optopt("", "onstop", "Command to run on song stop.", "COMMAND");
    opts.optflag("v", "verbose", "Add debug information to log.");
    opts.optflag("", "no-daemon", "Don't detach from console.");
    opts.optflag("", "backends", "List available audio backends.");
    opts.optflag("h", "help", "Print this help text.");
    opts
}

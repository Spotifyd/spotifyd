use std::convert::From;
use std::error::Error;
use std::panic;
use std::path::PathBuf;
use std::process::exit;

use daemonize::Daemonize;
use log::{error, info, LevelFilter};
use tokio_core::reactor::Core;

#[cfg(feature = "alsa_backend")]
mod alsa_mixer;
mod cli;
mod config;
#[cfg(feature = "dbus_mpris")]
mod dbus_mpris;
mod main_loop;
mod player_event_handler;
mod setup;
#[macro_use]
mod macros;

fn main() {
    let opts = cli::command_line_argument_options();
    let args: Vec<String> = std::env::args().collect();

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("Error: {}\n{}", f.to_string(), cli::usage(&args[0], &opts));
            exit(1)
        }
    };

    if matches.opt_present("backends") {
        cli::print_backends();
        exit(0);
    }

    if matches.opt_present("help") {
        println!("{}", cli::usage(&args[0], &opts));
        exit(0);
    }

    if matches.opt_present("version") {
        println!("spotifyd version {}", crate_version!());
        exit(0)
    }

    let config_file = matches
        .opt_str("config")
        .map(PathBuf::from)
        .or_else(|| config::get_config_file().ok());
    let config = config::get_config(config_file, &matches);

    if matches.opt_present("no-daemon") {
        let filter = if matches.opt_present("verbose") {
            simplelog::LogLevelFilter::Trace
        } else {
            simplelog::LogLevelFilter::Info
        };

        simplelog::TermLogger::init(filter, simplelog::Config::default())
            .map_err(Box::<Error>::from)
            .or_else(|_| {
                simplelog::SimpleLogger::init(filter, simplelog::Config::default())
                    .map_err(Box::<Error>::from)
            })
            .expect("Couldn't initialize logger");
    } else {
        let filter = if matches.opt_present("verbose") {
            LevelFilter::Trace
        } else {
            LevelFilter::Info
        };
        syslog::init(syslog::Facility::LOG_DAEMON, filter, Some("Spotifyd"))
            .expect("Couldn't initialize logger");

        let mut daemonize = Daemonize::new();
        if let Some(pid) = config.pid.as_ref() {
            daemonize = daemonize.pid_file(pid);
        }
        match daemonize.start() {
            Ok(_) => info!("Detached from shell, now running in background."),
            Err(e) => error!("Something went wrong while daemonizing: {}", e),
        };
    }

    panic::set_hook(Box::new(|panic_info| {
        error!(
            "Caught panic with message: {}",
            match (
                panic_info.payload().downcast_ref::<String>(),
                panic_info.payload().downcast_ref::<&str>(),
            ) {
                (Some(s), _) => &**s,
                (_, Some(&s)) => s,
                _ => "Unknown error type, can't produce message.",
            }
        );
    }));

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let initial_state = setup::initial_state(handle, config);

    core.run(initial_state).unwrap();
}

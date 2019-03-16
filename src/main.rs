#[cfg(feature = "alsa_backend")]
extern crate alsa;
extern crate chrono;
extern crate crypto;
extern crate daemonize;
#[cfg(feature = "dbus_mpris")]
extern crate dbus;
#[cfg(feature = "dbus_mpris")]
extern crate dbus_tokio;
extern crate futures;
extern crate getopts;
extern crate hostname;
extern crate ini;
extern crate librespot;
#[macro_use]
extern crate log;
#[cfg(feature = "dbus_mpris")]
extern crate rspotify;
extern crate simplelog;
extern crate syslog;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_signal;
extern crate xdg;

use std::process::exit;
use std::panic;
use std::convert::From;
use std::error::Error;

use daemonize::Daemonize;
use tokio_core::reactor::Core;

mod config;
mod cli;
#[cfg(feature = "alsa_backend")]
mod alsa_mixer;
mod main_loop;
mod setup;
mod player_event_handler;
#[cfg(feature = "dbus_mpris")]
mod dbus_mpris;
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
            log::LogLevelFilter::Trace
        } else {
            log::LogLevelFilter::Info
        };
        syslog::init(syslog::Facility::LOG_DAEMON, filter, Some("Spotifyd"))
            .expect("Couldn't initialize logger");

        let mut daemonize = Daemonize::new();
        if let Some(pid) = matches.opt_str("pid") {
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

    let initial_state = setup::initial_state(handle, &matches);

    core.run(initial_state).unwrap();
}

#![cfg(unix)]

use daemonize::Daemonize;
use log::{error, info, trace, LevelFilter};
use structopt::StructOpt;
use tokio_core::reactor::Core;

use std::{convert::From, error::Error, panic};

use crate::config::CliConfig;

#[cfg(feature = "alsa_backend")]
mod alsa_mixer;
mod config;
#[cfg(feature = "dbus_mpris")]
mod dbus_mpris;
mod error;
mod main_loop;
mod process;
mod setup;
mod utils;

fn main() {
    let mut cli_config = CliConfig::from_args();
    cli_config.load_config_file_values();

    let is_daemon = cli_config.daemon;
    let is_verbose = cli_config.verbose;

    if is_daemon {
        let filter = if is_verbose {
            LevelFilter::Trace
        } else {
            LevelFilter::Info
        };

        syslog::init(syslog::Facility::LOG_DAEMON, filter, Some("Spotifyd"))
            .expect("Couldn't initialize logger");
    } else {
        let filter = if is_verbose {
            simplelog::LogLevelFilter::Trace
        } else {
            simplelog::LogLevelFilter::Info
        };

        simplelog::TermLogger::init(filter, simplelog::Config::default())
            .map_err(Box::<dyn Error>::from)
            .or_else(|_| {
                simplelog::SimpleLogger::init(filter, simplelog::Config::default())
                    .map_err(Box::<dyn Error>::from)
            })
            .expect("Couldn't initialize logger");
    }

    if is_verbose {
        trace!("{:?}", &cli_config);
    }

    let internal_config = config::get_internal_config(cli_config);

    if is_daemon {
        let mut daemonize = Daemonize::new();
        if let Some(pid) = internal_config.pid.as_ref() {
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

    let initial_state = setup::initial_state(handle, internal_config);
    core.run(initial_state).unwrap();
}

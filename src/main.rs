#![cfg(unix)]

use daemonize::Daemonize;
use log::{error, info, trace, LevelFilter};
use simplelog::{ConfigBuilder, LevelPadding, SimpleLogger, TermLogger, TerminalMode};
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

fn setup_logger(is_daemon: bool, is_verbose: bool) {
    let filter = if is_verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };

    if is_daemon {
        syslog::init(syslog::Facility::LOG_DAEMON, filter, Some("Spotifyd"))
            .expect("Couldn't initialize logger");
    } else {
        let logger_config = ConfigBuilder::new()
            .set_level_padding(LevelPadding::Off)
            .build();

        TermLogger::init(filter, logger_config.clone(), TerminalMode::Mixed)
            .map_err(Box::<dyn Error>::from)
            .or_else(|_| SimpleLogger::init(filter, logger_config).map_err(Box::<dyn Error>::from))
            .expect("Couldn't initialize logger");
    }
}

fn main() {
    let mut cli_config: CliConfig = CliConfig::from_args();

    let is_daemon = !cli_config.no_daemon;
    let is_verbose = cli_config.verbose;
    setup_logger(is_daemon, is_verbose);

    cli_config.load_config_file_values();
    trace!("{:?}", &cli_config);

    // Returns the old SpotifydConfig struct used within the rest of the daemon.
    let internal_config = config::get_internal_config(cli_config);

    if is_daemon {
        info!("Daemonizing running instance");

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

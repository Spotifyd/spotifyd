#![cfg(unix)]

use crate::config::CliConfig;
use color_eyre::{
    eyre::{self, eyre, Context},
    Help, SectionExt,
};
#[cfg(unix)]
use daemonize::Daemonize;
use log::{error, info, trace, LevelFilter};
use structopt::StructOpt;
use tokio::runtime::Runtime;

#[cfg(feature = "alsa_backend")]
mod alsa_mixer;
mod config;
#[cfg(feature = "dbus_mpris")]
mod dbus_mpris;
mod error;
mod main_loop;
mod no_mixer;
mod process;
mod setup;
mod utils;

enum LogTarget {
    Terminal,
    #[cfg(unix)]
    Syslog,
}

fn setup_logger(log_target: LogTarget, verbose: bool) -> eyre::Result<()> {
    let log_level = if verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };

    let mut logger = fern::Dispatch::new().level(log_level);

    if cfg!(feature = "dbus_mpris") && !verbose {
        logger = logger.level_for("rspotify_http", LevelFilter::Warn);
    }

    let logger = match log_target {
        LogTarget::Terminal => logger.chain(std::io::stdout()),
        #[cfg(unix)]
        LogTarget::Syslog => {
            let log_format = syslog::Formatter3164 {
                facility: syslog::Facility::LOG_DAEMON,
                hostname: None,
                process: "spotifyd".to_owned(),
                pid: 0,
            };
            logger.chain(
                syslog::unix(log_format)
                    .map_err(|e| eyre!("Couldn't connect to syslog instance: {}", e))?,
            )
        }
    };

    logger.apply().wrap_err("Couldn't initialize logger")
}

fn main() -> eyre::Result<()> {
    color_eyre::install().wrap_err("Couldn't initialize error reporting")?;

    let mut cli_config: CliConfig = CliConfig::from_args();

    let is_daemon = !cli_config.no_daemon;

    let log_target = if is_daemon {
        #[cfg(unix)]
        {
            LogTarget::Syslog
        }
        #[cfg(target_os = "windows")]
        {
            LogTarget::Terminal
        }
    } else {
        LogTarget::Terminal
    };

    setup_logger(log_target, cli_config.verbose)?;

    cli_config
        .load_config_file_values()
        .wrap_err("could not load the config file")
        .with_section(|| {
            concat!(
                "the config format should be valid TOML\n",
                "we recently changed the config format, see https://github.com/Spotifyd/spotifyd/issues/765"
            )
            .header("note:")
        })?;
    trace!("{:?}", &cli_config);

    // Returns the old SpotifydConfig struct used within the rest of the daemon.
    let internal_config = config::get_internal_config(cli_config);
    println!(
        "{:?} {:?}",
        internal_config.username, internal_config.password
    );

    #[cfg(unix)]
    {
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
    }

    let runtime = Runtime::new().unwrap();
    runtime.block_on(async {
        let mut initial_state = setup::initial_state(internal_config);
        initial_state.run().await;
    });

    Ok(())
}

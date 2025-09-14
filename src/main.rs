use crate::config::CliConfig;
use clap::Parser;
#[cfg(unix)]
use color_eyre::eyre::eyre;
use color_eyre::eyre::{self, Context};
use config::ExecutionMode;
#[cfg(unix)]
use daemonize::Daemonize;
use fern::colors::ColoredLevelConfig;
use log::{LevelFilter, info, trace};
use oauth::run_oauth;
#[cfg(target_os = "openbsd")]
use pledge::pledge;
#[cfg(windows)]
use std::fs;
use tokio::runtime::Runtime;

#[cfg(feature = "alsa_backend")]
mod alsa_mixer;
mod config;
#[cfg(feature = "dbus_mpris")]
mod dbus_mpris;
mod error;
mod main_loop;
mod no_mixer;
mod oauth;
mod process;
mod setup;
mod utils;

enum LogTarget {
    Terminal,
    Syslog,
}

fn setup_logger(log_target: LogTarget, verbose: u8) -> eyre::Result<()> {
    let log_level = match verbose {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        2.. => LevelFilter::Trace,
    };

    let mut logger = fern::Dispatch::new().level(log_level);

    if verbose == 0 {
        logger = logger.level_for("symphonia_format_ogg::demuxer", LevelFilter::Warn);
    }

    let logger = match log_target {
        LogTarget::Terminal => {
            let colors = ColoredLevelConfig::new();
            logger
                .format(move |out, message, record| {
                    let target = if verbose > 0 {
                        &format!(" {}", record.target())
                    } else {
                        ""
                    };
                    out.finish(format_args!(
                        "[{}{}] {}",
                        colors.color(record.level()),
                        target,
                        message
                    ))
                })
                .chain(std::io::stdout())
        }
        #[cfg(unix)]
        LogTarget::Syslog => {
            let log_format = syslog::Formatter3164 {
                facility: syslog::Facility::LOG_DAEMON,
                hostname: None,
                process: "spotifyd".to_owned(),
                pid: std::process::id(),
            };
            logger.chain(
                syslog::unix(log_format)
                    .map_err(|e| eyre!("Couldn't connect to syslog instance: {}", e))?,
            )
        }
        #[cfg(target_os = "windows")]
        LogTarget::Syslog => {
            let dirs = directories::BaseDirs::new().unwrap();
            let mut log_file = dirs.data_local_dir().to_path_buf();
            log_file.push("spotifyd");
            log_file.push(".spotifyd.log");

            if let Some(p) = log_file.parent() {
                fs::create_dir_all(p)?
            };
            logger.chain(
                fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(log_file)
                    .expect("Couldn't initialize logger"),
            )
        }
    };

    logger.apply().wrap_err("Couldn't initialize logger")
}

fn main() -> eyre::Result<()> {
    // Start with superset of all potentially required promises.
    // Drop later after CLI arguments and configuration files were parsed.
    #[cfg(target_os = "openbsd")]
    pledge(
        "stdio rpath wpath cpath inet mcast flock chown unix dns proc exec audio",
        None,
    )
    .unwrap();

    color_eyre::install().wrap_err("Couldn't initialize error reporting")?;

    let cli_config = CliConfig::parse();

    match cli_config.mode {
        None => run_daemon(cli_config),
        Some(ExecutionMode::Authenticate { oauth_port }) => run_oauth(cli_config, oauth_port),
    }
}

fn run_daemon(mut cli_config: CliConfig) -> eyre::Result<()> {
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
        #[cfg(unix)]
        {
            LogTarget::Terminal
        }
        #[cfg(target_os = "windows")]
        {
            if std::env::var("SPOTIFYD_CHILD").is_ok() {
                LogTarget::Syslog
            } else {
                LogTarget::Terminal
            }
        }
    };

    setup_logger(log_target, cli_config.verbose)?;

    cli_config
        .load_config_file_values()
        .wrap_err("could not load the config file")?;
    trace!("{:?}", &cli_config);

    // Returns the old SpotifydConfig struct used within the rest of the daemon.
    let internal_config = config::get_internal_config(cli_config);

    if is_daemon {
        info!("Daemonizing running instance");

        #[cfg(unix)]
        {
            let mut daemonize = Daemonize::new();
            if let Some(pid) = internal_config.pid.as_ref() {
                daemonize = daemonize.pid_file(pid);
            }
            match daemonize.start() {
                Ok(_) => info!("Detached from shell, now running in background."),
                Err(e) => return Err(e).wrap_err("something went wrong while daemonizing"),
            };
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            use std::process::{Command, exit};

            let mut args = std::env::args().collect::<Vec<_>>();
            args.remove(0);
            args.push("--no-daemon".to_string());

            Command::new(std::env::current_exe().unwrap())
                .args(args)
                .env("SPOTIFYD_CHILD", "1")
                .creation_flags(8 /* DETACHED_PROCESS */)
                .spawn()
                .expect("Couldn't spawn daemon");

            exit(0);
        }
    }

    #[cfg(target_os = "openbsd")]
    {
        // At this point:
        //   * --username-cmd, --password-cmd were handled
        //     > no "proc exec"
        //   * --pid, daemon(3) were handled
        //     > no "cpath flock chown" for PID file
        //     > no "proc" for double-fork(2)
        //
        // Required runtime promises:
        // stdout/err, syslog(3)    "stdio"
        // ${TMPDIR}/.tmp*, cache   "[rwc]path"
        // Spotify API/Connect      "inet dns"
        // D-Bus, MPRIS             "unix"
        // Zeroconf Discovery       "mcast"
        // PortAudio, sio_open(3)  ("[rwc]path unix inet audio")
        // > after sndio(7) cookie  "audio"

        // --on-song-change-hook aka. "onevent", run via --shell aka. "shell"
        if internal_config.onevent.is_some() {
            pledge(
                "stdio rpath wpath cpath inet mcast unix dns proc exec audio",
                None,
            )
            .unwrap();
        } else {
            pledge("stdio rpath wpath cpath inet mcast unix dns audio", None).unwrap();
        }
    }

    let runtime = Runtime::new().unwrap();
    runtime.block_on(async {
        let initial_state = setup::initial_state(internal_config)?;
        initial_state.run().await
    })
}

extern crate daemonize;
extern crate getopts;
extern crate simplelog;
extern crate rpassword;
extern crate librespot;
extern crate ini;
extern crate xdg;
extern crate syslog;
#[macro_use]
extern crate log;
extern crate futures;
extern crate tokio_core;

use std::process::exit;
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::convert::From;
use std::error::Error;
use std::path::PathBuf;
use std::io;

use librespot::spirc::{Spirc, SpircTask};
use librespot::session::Session;
use librespot::player::Player;
use librespot::audio_backend::{BACKENDS, Sink};
use librespot::authentication::get_credentials;
use librespot::mixer;
use librespot::cache::Cache;

use daemonize::Daemonize;
use futures::{Future, Async, Poll};
use tokio_core::reactor::Core;

mod config;
mod cli;

struct MainLoopState {
    connection: Box<Future<Item=Session, Error=io::Error>>,
    mixer: fn() -> Box<mixer::Mixer>,
    backend: fn(Option<String>) -> Box<Sink>,
    device_name: Option<String>,
    spirc_task: Option<SpircTask>,
    spirc: Option<Spirc>,
}

impl Future for MainLoopState {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        if let Async::Ready(session) = self.connection.poll().unwrap() {
            let audio_filter = (self.mixer)().get_audio_filter();
            self.connection = Box::new(futures::future::empty());
            let backend = self.backend;
            let device_name = self.device_name.clone();
            let player = Player::new(session.clone(), audio_filter, move || {
                (backend)(device_name)
            });

            let (spirc, spirc_task) = Spirc::new(session, player, (self.mixer)());
            self.spirc_task = Some(spirc_task);
            self.spirc = Some(spirc);
        }
        if let Some(ref mut spirc_task) = self.spirc_task {
            if let Ok(Async::Ready(())) = spirc_task.poll() {
                // TODO: Shutdown.
            }
        }
        Ok(Async::NotReady)
    }
}

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
        error!("Caught panic with message: {}",
               match (panic_info.payload().downcast_ref::<String>(),
                      panic_info.payload().downcast_ref::<&str>()) {
                   (Some(s), _) => &**s,
                   (_, Some(&s)) => s,
                   _ => "Unknown error type, can't produce message.",
               });
    }));

    let config_file = matches.opt_str("config")
        .map(|s| PathBuf::from(s))
        .or_else(|| config::get_config_file().ok());
    let config = config::get_config(config_file);

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let cache = config.cache;
    let session_config = config.session_config;
    let backend = config.backend.clone();
    let device_name = config.device.clone();
    let mut connection =
        Box::new(futures::future::empty()) as Box<futures::Future<Item = Session,
                                                                  Error = io::Error>>;
    if let Some(credentials) =
        get_credentials(config.username.or(matches.opt_str("username")),
                        config.password.or(matches.opt_str("password")),
                        cache.as_ref().and_then(Cache::credentials)) {
        connection = Session::connect(session_config, credentials, cache, handle);
    }

    let mixer = mixer::find(None as Option<String>).unwrap();
    let backend = find_backend(backend.as_ref().map(String::as_ref));
    let initial_state = MainLoopState { connection, mixer, backend, device_name, spirc_task: None, spirc: None };
    core.run(initial_state).unwrap();
}

fn find_backend(name: Option<&str>) -> fn(Option<String>) -> Box<Sink> {
    match name {
        Some(name) => {
            BACKENDS.iter()
                .find(|backend| name == backend.0)
                .expect(format!("Unknown backend: {}.", name).as_ref())
                .1
        }
        None => {
            let &(name, back) = BACKENDS.first().expect("No backends were enabled at build time");
            info!("No backend specified, defaulting to: {}.", name);
            back
        }
    }
}

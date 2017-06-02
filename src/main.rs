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
extern crate tokio_io;
extern crate tokio_signal;

use std::process::exit;
use std::panic;
use std::convert::From;
use std::error::Error;
use std::path::PathBuf;
use std::io;

use librespot::spirc::{Spirc, SpircTask};
use librespot::session::{Session, Config as SessionConfig};
use librespot::player::Player;
use librespot::audio_backend::{BACKENDS, Sink};
use librespot::authentication::get_credentials;
use librespot::authentication::discovery::{discovery, DiscoveryStream};
use librespot::mixer;
use librespot::cache::Cache;

use daemonize::Daemonize;
use futures::{Future, Async, Poll, Stream};
use tokio_core::reactor::{Handle, Core};
use tokio_io::IoStream;
use tokio_signal::ctrl_c;

mod config;
mod cli;

struct MainLoopState {
    connection: Box<Future<Item = Session, Error = io::Error>>,
    mixer: fn() -> Box<mixer::Mixer>,
    backend: fn(Option<String>) -> Box<Sink>,
    audio_device: Option<String>,
    spirc_task: Option<SpircTask>,
    spirc: Option<Spirc>,
    ctrl_c_stream: IoStream<()>,
    shutting_down: bool,
    cache: Option<Cache>,
    config: SessionConfig,
    handle: Handle,
    discovery_stream: DiscoveryStream,
}

impl MainLoopState {
    fn new(connection: Box<Future<Item = Session, Error = io::Error>>,
           mixer: fn() -> Box<mixer::Mixer>,
           backend: fn(Option<String>) -> Box<Sink>,
           audio_device: Option<String>,
           ctrl_c_stream: IoStream<()>,
           discovery_stream: DiscoveryStream,
           cache: Option<Cache>,
           config: SessionConfig,
           handle: Handle)
           -> MainLoopState {
        MainLoopState {
            connection: connection,
            mixer: mixer,
            backend: backend,
            audio_device: audio_device,
            spirc_task: None,
            spirc: None,
            ctrl_c_stream: ctrl_c_stream,
            shutting_down: false,
            cache: cache,
            config: config,
            handle: handle,
            discovery_stream: discovery_stream,
        }
    }
}


impl Future for MainLoopState {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            if let Async::Ready(Some(creds)) = self.discovery_stream.poll().unwrap() {
                if let Some(ref mut spirc) = self.spirc {
                    spirc.shutdown();
                }
                let config = self.config.clone();
                let cache = self.cache.clone();
                let handle = self.handle.clone();
                self.connection = Session::connect(config, creds, cache, handle);
            }

            if let Async::Ready(session) = self.connection.poll().unwrap() {
                let audio_filter = (self.mixer)().get_audio_filter();
                self.connection = Box::new(futures::future::empty());
                let backend = self.backend;
                let audio_device = self.audio_device.clone();
                let player = Player::new(session.clone(),
                                         audio_filter,
                                         move || (backend)(audio_device));

                let (spirc, spirc_task) = Spirc::new("Spotifyd".to_string(), session, player, (self.mixer)());
                self.spirc_task = Some(spirc_task);
                self.spirc = Some(spirc);
            } else if let Async::Ready(_) = self.ctrl_c_stream.poll().unwrap() {
                if !self.shutting_down {
                    if let Some(ref spirc) = self.spirc {
                        spirc.shutdown();
                        self.shutting_down = true;
                    } else {
                        return Ok(Async::Ready(()));
                    }
                }
            } else if let Some(Async::Ready(_)) =
                self.spirc_task
                    .as_mut()
                    .map(|ref mut st| st.poll().unwrap()) {
                return Ok(Async::Ready(()));
            } else {
                return Ok(Async::NotReady);
            }
        }
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
    let config = config::get_config(config_file, &matches);

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let cache = config.cache;
    let session_config = config.session_config;
    let backend = config.backend.clone();
    let audio_device = config.audio_device.clone();
    let device_id = session_config.device_id.clone();
    let discovery_stream = discovery(&handle, "Spotifyd".to_string(), device_id).unwrap();
    let connection = if let Some(credentials) =
        get_credentials(config.username.or(matches.opt_str("username")),
                        config.password.or(matches.opt_str("password")),
                        cache.as_ref().and_then(Cache::credentials)) {
        Session::connect(session_config.clone(),
                         credentials,
                         cache.clone(),
                         handle.clone())
    } else {
        Box::new(futures::future::empty()) as Box<futures::Future<Item = Session,
                                                                  Error = io::Error>>
    };

    let mixer = mixer::find(None as Option<String>).unwrap();
    let backend = find_backend(backend.as_ref().map(String::as_ref));
    let initial_state = MainLoopState::new(connection,
                                           mixer,
                                           backend,
                                           audio_device,
                                           ctrl_c(&handle).flatten_stream().boxed(),
                                           discovery_stream,
                                           cache,
                                           session_config,
                                           handle);
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

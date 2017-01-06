extern crate daemonize;
extern crate getopts;
extern crate simplelog;
extern crate librespot;
extern crate ini;
extern crate xdg;
extern crate syslog;
#[macro_use]
extern crate log;
extern crate ctrlc;

use std::process::exit;
use std::thread;
use std::panic;

use librespot::spirc::SpircManager;
use librespot::main_helper;
use librespot::session::Session;
use librespot::player::Player;

use daemonize::Daemonize;

mod config;
mod cli;

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
            .expect("Couldn't initialize logger.");
    } else {
        let filter = if matches.opt_present("verbose") {
            log::LogLevelFilter::Trace
        } else {
            log::LogLevelFilter::Info
        };
        syslog::init(syslog::Facility::LOG_DAEMON, filter, Some("Spotifyd"))
            .expect("Couldn't initialize logger.");

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
        error!("Received panic with payload: {:?}", panic_info.payload())
    }));

    let config = config::get_config();

    let cache = config.cache;
    let backend = config.backend;
    let session_config = config.session_config;
    let device_name = session_config.device_name.clone();
    let session = Session::new(session_config, cache);
    let credentials = main_helper::get_credentials(&session, &matches);
    session.login(credentials).unwrap();
    let player = Player::new(session.clone(), move || {
        main_helper::find_backend(backend.as_ref()
            .map(AsRef::as_ref))(Some(device_name.as_ref()))
    });

    let spirc = SpircManager::new(session.clone(), player);
    let spirc_signal = spirc.clone();
    thread::spawn(move || spirc.run());

    ctrlc::set_handler(move || {
        info!("Signal received. Say goodbye and exit.");
        spirc_signal.send_goodbye();
        exit(0);
    });

    loop {
        session.poll();
    }

}

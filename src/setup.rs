use std::io;
use std::path::PathBuf;
use std::process::exit;
use tokio_core::reactor::Handle;
use tokio_signal::ctrl_c;
use librespot::core::session::Session;
use librespot::core::authentication::get_credentials;
use librespot::connect::discovery::discovery;
use librespot::playback::mixer::Mixer;
use librespot::playback::mixer;
use librespot::core::config::{ConnectConfig, DeviceType};
use librespot::core::cache::Cache;
use librespot::playback::audio_backend::{Sink, BACKENDS};
use futures::Future;
use getopts::Matches;
use config;
#[cfg(feature = "alsa_backend")]
use alsa_mixer;
use futures;
use main_loop;

pub fn initial_state(handle: Handle, matches: &Matches) -> main_loop::MainLoopState {
    let config_file = matches
        .opt_str("config")
        .map(PathBuf::from)
        .or_else(|| config::get_config_file().ok());
    let config = config::get_config(config_file, matches);

    let local_audio_device = config.audio_device.clone();
    let local_mixer = config.mixer.clone();

    #[cfg(feature = "alsa_backend")]
    let mut mixer = match config.volume_controller {
        config::VolumeController::Alsa { linear } => {
            info!("Using alsa volume controller.");
            Box::new(move || {
                Box::new(alsa_mixer::AlsaMixer {
                    device: local_audio_device
                        .clone()
                        .unwrap_or_else(|| "default".to_string()),
                    mixer: local_mixer.clone().unwrap_or_else(|| "Master".to_string()),
                    linear_scaling: linear,
                }) as Box<mixer::Mixer>
            }) as Box<FnMut() -> Box<Mixer>>
        }
        config::VolumeController::SoftVol => {
            info!("Using software volume controller.");
            Box::new(|| Box::new(mixer::softmixer::SoftMixer::open()) as Box<Mixer>)
                as Box<FnMut() -> Box<Mixer>>
        }
    };

    #[cfg(not(feature = "alsa_backend"))]
    let mut mixer = {
        info!("Using software volume controller.");
        Box::new(|| Box::new(mixer::softmixer::SoftMixer::open()) as Box<Mixer>)
            as Box<FnMut() -> Box<Mixer>>
    };

    let cache = config.cache;
    let player_config = config.player_config;
    let session_config = config.session_config;
    let backend = config.backend.clone();
    let device_id = session_config.device_id.clone();

    #[cfg(feature = "alsa_backend")]
    let linear_volume = match config.volume_controller {
        config::VolumeController::Alsa { linear } => linear,
        _ => false,
    };

    #[cfg(not(feature = "alsa_backend"))]
    let linear_volume = false;

    let discovery_stream = discovery(
        &handle,
        ConnectConfig {
            name: matches.opt_str("device_name").unwrap_or(config.device_name.clone()),
            device_type: DeviceType::default(),
            volume: u16::from((mixer()).volume()),
            linear_volume: linear_volume,
        },
        device_id,
        0,
    ).unwrap();
    let connection = if let Some(credentials) = get_credentials(
        config.username.or_else(|| matches.opt_str("username")),
        config.password.or_else(|| matches.opt_str("password")),
        cache.as_ref().and_then(Cache::credentials),
        |_| {
            error!("No password found.");
            exit(1);
        },
    ) {
        Session::connect(
            session_config.clone(),
            credentials,
            cache.clone(),
            handle.clone(),
        )
    } else {
        Box::new(futures::future::empty())
            as Box<futures::Future<Item = Session, Error = io::Error>>
    };

    let backend = find_backend(backend.as_ref().map(String::as_ref));
    main_loop::MainLoopState {
        librespot_connection: main_loop::LibreSpotConnection::new(connection, discovery_stream),
        audio_setup: main_loop::AudioSetup {
            mixer: mixer,
            backend: backend,
            audio_device: config.audio_device.clone(),
        },
        spotifyd_state: main_loop::SpotifydState {
            ctrl_c_stream: Box::new(ctrl_c(&handle).flatten_stream()),
            shutting_down: false,
            cache: cache,
            device_name: matches.opt_str("device_name").unwrap_or(config.device_name.clone()),
            player_event_channel: None,
            player_event_program: config.onevent,
            dbus_mpris_server: None,
        },
        player_config: player_config,
        session_config: session_config,
        handle: handle,
        linear_volume: linear_volume,
    }
}

fn find_backend(name: Option<&str>) -> fn(Option<String>) -> Box<Sink> {
    match name {
        Some(name) => {
            BACKENDS
                .iter()
                .find(|backend| name == backend.0)
                .expect(format!("Unknown backend: {}.", name).as_ref())
                .1
        }
        None => {
            let &(name, back) = BACKENDS
                .first()
                .expect("No backends were enabled at build time");
            info!("No backend specified, defaulting to: {}.", name);
            back
        }
    }
}

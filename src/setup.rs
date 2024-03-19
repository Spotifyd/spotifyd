#[cfg(feature = "alsa_backend")]
use crate::alsa_mixer;
use crate::{
    config,
    main_loop::{self, CredentialsProvider},
};
#[cfg(feature = "dbus_keyring")]
use keyring::Entry;
use librespot_core::{authentication::Credentials, cache::Cache, config::DeviceType};
use librespot_playback::mixer::MixerConfig;
use librespot_playback::{
    audio_backend::{Sink, BACKENDS},
    config::AudioFormat,
    mixer::{self, Mixer},
};
#[allow(unused_imports)] // cfg
use log::{debug, error, info, warn};
use std::{str::FromStr, thread, time::Duration};

pub(crate) fn initial_state(config: config::SpotifydConfig) -> main_loop::MainLoop {
    let mixer = {
        match config.volume_controller {
            config::VolumeController::None => {
                info!("Using no volume controller.");
                Box::new(|| Box::new(crate::no_mixer::NoMixer) as Box<dyn Mixer>)
                    as Box<dyn FnMut() -> Box<dyn Mixer>>
            }
            #[cfg(feature = "alsa_backend")]
            config::VolumeController::Alsa | config::VolumeController::AlsaLinear => {
                let audio_device = config.audio_device.clone();
                let control_device = config.control_device.clone();
                let mixer = config.mixer.clone();
                info!("Using alsa volume controller.");
                let linear = matches!(
                    config.volume_controller,
                    config::VolumeController::AlsaLinear
                );
                Box::new(move || {
                    Box::new(alsa_mixer::AlsaMixer {
                        device: control_device
                            .clone()
                            .or_else(|| audio_device.clone())
                            .unwrap_or_else(|| "default".to_string()),
                        mixer: mixer.clone().unwrap_or_else(|| "Master".to_string()),
                        linear_scaling: linear,
                    }) as Box<dyn mixer::Mixer>
                }) as Box<dyn FnMut() -> Box<dyn Mixer>>
            }
            _ => {
                info!("Using software volume controller.");
                Box::new(move || {
                    Box::new(mixer::softmixer::SoftMixer::open(MixerConfig::default()))
                        as Box<dyn Mixer>
                }) as Box<dyn FnMut() -> Box<dyn Mixer>>
            }
        }
    };

    let cache = config.cache;
    let player_config = config.player_config;
    let session_config = config.session_config;
    let backend = config.backend.clone();
    let autoplay = config.autoplay;

    let has_volume_ctrl = !matches!(config.volume_controller, config::VolumeController::None);

    let zeroconf_port = config.zeroconf_port.unwrap_or(0);

    let device_type: DeviceType = DeviceType::from_str(&config.device_type).unwrap_or_default();

    let username = config.username;
    #[allow(unused_mut)] // mut is needed behind the dbus_keyring flag.
    let mut password = config.password;

    #[cfg(feature = "dbus_keyring")]
    if config.use_keyring {
        match (&username, &password) {
            (None, _) => warn!("Can't query the keyring without a username"),
            (Some(_), Some(_)) => {
                info!("Keyring is ignored, since you already configured a password")
            }
            (Some(username), None) => {
                info!("Checking keyring for password");
                let entry = Entry::new("spotifyd", username);
                match entry.and_then(|e| e.get_password()) {
                    Ok(retrieved_password) => password = Some(retrieved_password),
                    Err(e) => error!("Keyring did not return any results: {e}"),
                }
            }
        }
    }

    let credentials_provider =
        if let Some(credentials) = get_credentials(&cache, &username, &password) {
            CredentialsProvider::SpotifyCredentials(credentials)
        } else {
            info!("no usable credentials found, enabling discovery");
            debug!("Using device id '{}'", session_config.device_id);
            const RETRY_MAX: u8 = 4;
            let mut retry_counter = 0;
            let mut backoff = Duration::from_secs(5);
            let discovery_stream = loop {
                match librespot_discovery::Discovery::builder(session_config.device_id.clone())
                    .name(config.device_name.clone())
                    .device_type(device_type)
                    .port(zeroconf_port)
                    .launch()
                {
                    Ok(discovery_stream) => break discovery_stream,
                    Err(err) => {
                        error!("failed to enable discovery: {err}");
                        if retry_counter >= RETRY_MAX {
                            panic!("failed to enable discovery (and no credentials provided)");
                        }
                        info!("retrying discovery in {} seconds", backoff.as_secs());
                        thread::sleep(backoff);
                        retry_counter += 1;
                        backoff *= 2;
                        info!("trying to enable discovery (retry {retry_counter}/{RETRY_MAX})");
                    }
                }
            };
            discovery_stream.into()
        };

    let backend = find_backend(backend.as_ref().map(String::as_ref));
    main_loop::MainLoop {
        credentials_provider,
        audio_setup: main_loop::AudioSetup {
            mixer,
            backend,
            audio_device: config.audio_device,
            audio_format: config.audio_format,
        },
        spotifyd_state: main_loop::SpotifydState {
            cache,
            device_name: config.device_name,
            player_event_program: config.onevent,
        },
        player_config,
        session_config,
        initial_volume: config.initial_volume,
        has_volume_ctrl,
        shell: config.shell,
        device_type,
        autoplay,
        use_mpris: config.use_mpris,
        dbus_type: config.dbus_type,
    }
}

fn get_credentials(
    cache: &Option<Cache>,
    username: &Option<String>,
    password: &Option<String>,
) -> Option<Credentials> {
    if let Some(credentials) = cache.as_ref().and_then(Cache::credentials) {
        if username.as_ref() == Some(&credentials.username) {
            return Some(credentials);
        }
    }

    Some(Credentials::with_password(
        username.as_ref()?,
        password.as_ref()?,
    ))
}

fn find_backend(name: Option<&str>) -> fn(Option<String>, AudioFormat) -> Box<dyn Sink> {
    match name {
        Some(name) => {
            BACKENDS
                .iter()
                .find(|backend| name == backend.0)
                .unwrap_or_else(|| panic!("Unknown backend: {}.", name))
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

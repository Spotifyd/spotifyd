#[cfg(feature = "alsa_backend")]
use crate::alsa_mixer;
use crate::{
    config,
    main_loop::{self, CredentialsProvider},
    utils::Backoff,
};
use color_eyre::{eyre::eyre, Section};
use futures::StreamExt as _;
use librespot_playback::{
    audio_backend::{self},
    mixer::{self, Mixer, MixerConfig},
};
use log::{debug, error, info};
use std::{sync::Arc, thread};

pub(crate) fn initial_state(
    config: config::SpotifydConfig,
) -> color_eyre::Result<main_loop::MainLoop> {
    let mixer: Arc<dyn Mixer> = {
        match config.volume_controller {
            config::VolumeController::None => {
                info!("Using no volume controller.");
                Arc::new(crate::no_mixer::NoMixer)
            }
            #[cfg(feature = "alsa_backend")]
            config::VolumeController::Alsa | config::VolumeController::AlsaLinear => {
                let audio_device = config.audio_device.clone();
                let control_device = config.alsa_config.control.clone();
                let mixer = config.alsa_config.mixer.clone();
                info!("Using alsa volume controller.");
                let linear = matches!(
                    config.volume_controller,
                    config::VolumeController::AlsaLinear
                );
                Arc::new(alsa_mixer::AlsaMixer {
                    device: control_device
                        .clone()
                        .or_else(|| audio_device.clone())
                        .unwrap_or_else(|| "default".to_string()),
                    mixer: mixer.clone().unwrap_or_else(|| "Master".to_string()),
                    linear_scaling: linear,
                })
            }
            _ => {
                info!("Using software volume controller.");
                Arc::new(mixer::softmixer::SoftMixer::open(MixerConfig::default())?)
            }
        }
    };

    let player_config = config.player_config;
    let session_config = config.session_config;
    let backend = config.backend.clone();

    let has_volume_ctrl = !matches!(config.volume_controller, config::VolumeController::None);

    let zeroconf_port = config.zeroconf_port.unwrap_or(0);

    let creds = if let Some(creds) = config.oauth_cache.as_ref().and_then(|c| c.credentials()) {
        info!(
            "Login via OAuth as user {}.",
            creds.username.as_deref().unwrap_or("unknown")
        );
        Some(creds)
    } else if let Some(creds) = config.cache.as_ref().and_then(|c| c.credentials()) {
        info!(
            "Restoring previous login as user {}.",
            creds.username.as_deref().unwrap_or("unknown")
        );
        Some(creds)
    } else {
        None
    };

    let discovery = if config.discovery {
        info!("Starting zeroconf server to advertise on local network.");
        debug!("Using device id '{}'", session_config.device_id);
        let mut retry_backoff = Backoff::default();
        loop {
            match librespot_discovery::Discovery::builder(
                session_config.device_id.clone(),
                session_config.client_id.clone(),
            )
            .name(config.device_name.clone())
            .device_type(config.device_type)
            .port(zeroconf_port)
            .launch()
            {
                Ok(discovery_stream) => break Some(discovery_stream),
                Err(err) => {
                    error!("failed to enable discovery: {err}");
                    let Ok(backoff) = retry_backoff.next_backoff() else {
                        error!("maximum amount of retries exceeded");
                        break None;
                    };
                    info!("retrying discovery in {} seconds", backoff.as_secs());
                    thread::sleep(backoff);
                    info!(
                        "trying to enable discovery (retry {}/{})",
                        retry_backoff.retries(),
                        retry_backoff.max_retries()
                    );
                }
            }
        }
    } else {
        None
    };

    let credentials_provider = match (discovery, creds) {
        (Some(stream), creds) => CredentialsProvider::Discovery {
            stream: stream.peekable(),
            last_credentials: creds,
        },
        (None, Some(creds)) => CredentialsProvider::CredentialsOnly(creds),
        (None, None) => {
            return Err(
                eyre!("Discovery unavailable and no credentials found.").with_suggestion(|| {
                    "Try enabling discovery or logging in first with `spotifyd authenticate`."
                }),
            );
        }
    };

    let backend = audio_backend::find(backend).expect("available backends should match ours");

    Ok(main_loop::MainLoop {
        credentials_provider,
        mixer,
        session_config,
        cache: config.cache,
        audio_device: config.audio_device,
        audio_format: config.audio_format,
        player_config,
        backend,
        initial_volume: config.initial_volume,
        has_volume_ctrl,
        shell: config.shell,
        device_type: config.device_type,
        device_name: config.device_name,
        player_event_program: config.onevent,
        #[cfg(feature = "dbus_mpris")]
        mpris_config: config.mpris,
    })
}

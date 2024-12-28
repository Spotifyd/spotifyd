#[cfg(feature = "alsa_backend")]
use crate::alsa_mixer;
use crate::{
    config,
    main_loop::{self, CredentialsProvider},
};
use color_eyre::{
    eyre::{eyre, Context},
    Section,
};
use librespot_playback::{
    audio_backend::{self},
    mixer::{self, Mixer, MixerConfig},
};
use log::{debug, error, info};
use std::{sync::Arc, thread, time::Duration};

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
                Arc::new(mixer::softmixer::SoftMixer::open(MixerConfig::default()))
            }
        }
    };

    let cache = config.cache;
    let player_config = config.player_config;
    let session_config = config.session_config;
    let backend = config.backend.clone();

    let has_volume_ctrl = !matches!(config.volume_controller, config::VolumeController::None);

    let zeroconf_port = config.zeroconf_port.unwrap_or(0);

    let credentials_provider =
        if let Some(credentials) = cache.as_ref().and_then(|c| c.credentials()) {
            CredentialsProvider::SpotifyCredentials(credentials)
        } else if config.discovery {
            info!("no usable credentials found, enabling discovery");
            debug!("Using device id '{}'", session_config.device_id);
            const RETRY_MAX: u8 = 4;
            let mut retry_counter = 0;
            let mut backoff = Duration::from_secs(5);
            let discovery_stream = loop {
                match librespot_discovery::Discovery::builder(
                    session_config.device_id.clone(),
                    session_config.client_id.clone(),
                )
                .name(config.device_name.clone())
                .device_type(config.device_type)
                .port(zeroconf_port)
                .launch()
                {
                    Ok(discovery_stream) => break discovery_stream,
                    Err(err) => {
                        error!("failed to enable discovery: {err}");
                        if retry_counter >= RETRY_MAX {
                            return Err(err).with_context(|| {
                                "failed to enable discovery (and no credentials provided)"
                            });
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
        } else {
            return Err(eyre!(
                "no cached credentials available and discovery disabled"
            ))
            .with_suggestion(|| "consider enabling discovery or authenticating via OAuth");
        };

    let backend = audio_backend::find(backend).expect("available backends should match ours");

    Ok(main_loop::MainLoop {
        credentials_provider,
        mixer,
        session_config,
        cache,
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

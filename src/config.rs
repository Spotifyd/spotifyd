use getopts::Matches;
use hostname;
use ini::Ini;
use librespot::{
    core::{cache::Cache, config::SessionConfig, version},
    playback::config::{Bitrate, PlayerConfig},
};
use log::info;
use sha1::{Digest, Sha1};
use std::{
    convert::From,
    fs::metadata,
    mem::swap,
    path::{Path, PathBuf},
    str::FromStr,
};
use xdg;

use crate::error::{Error, ErrorKind};
use crate::process::run_program;

const CONFIG_FILE: &str = "spotifyd.conf";

pub enum VolumeController {
    Alsa { linear: bool },
    SoftVol,
}

fn device_id(name: &str) -> String {
    hex::encode(&Sha1::digest(name.as_bytes()))
}

impl FromStr for VolumeController {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &*s.to_uppercase() {
            "ALSA" => Ok(VolumeController::Alsa { linear: false }),
            "ALSA_LINEAR" => Ok(VolumeController::Alsa { linear: true }),
            "SOFTVOL" => Ok(VolumeController::SoftVol),
            _ => Err(()),
        }
    }
}

pub(crate) struct SpotifydConfig {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) use_keyring: bool,
    pub(crate) cache: Option<Cache>,
    pub(crate) backend: Option<String>,
    pub(crate) audio_device: Option<String>,
    pub(crate) control_device: Option<String>,
    pub(crate) mixer: Option<String>,
    pub(crate) volume_controller: VolumeController,
    pub(crate) device_name: String,
    pub(crate) player_config: PlayerConfig,
    pub(crate) session_config: SessionConfig,
    pub(crate) onevent: Option<String>,
    pub(crate) pid: Option<String>,
}

impl Default for SpotifydConfig {
    fn default() -> SpotifydConfig {
        SpotifydConfig {
            username: None,
            password: None,
            use_keyring: false,
            cache: None,
            backend: None,
            audio_device: None,
            control_device: None,
            mixer: None,
            volume_controller: VolumeController::SoftVol,
            device_name: "Spotifyd".to_string(),
            player_config: PlayerConfig {
                bitrate: Bitrate::Bitrate160,
                normalisation: false,
                normalisation_pregain: 0.0,
            },
            session_config: SessionConfig {
                user_agent: version::version_string(),
                device_id: device_id("Spotifyd"),
                proxy: None,
                ap_port: Some(443),
            },
            onevent: None,
            pid: None,
        }
    }
}

pub(crate) fn get_config_file() -> Option<PathBuf> {
    let etc_conf = format!("/etc/{}", CONFIG_FILE);
    let xdg_dirs = xdg::BaseDirectories::with_prefix("spotifyd").ok()?;
    xdg_dirs.find_config_file(CONFIG_FILE).or_else(|| {
        metadata(&*etc_conf).ok().and_then(|meta| {
            if meta.is_file() {
                Some(etc_conf.into())
            } else {
                None
            }
        })
    })
}

fn update<T>(r: &mut T, val: Option<T>) {
    if let Some(mut v) = val {
        swap(r, &mut v);
    }
}

pub(crate) fn get_config<P: AsRef<Path>>(
    config_path: Option<P>,
    matches: &Matches,
) -> Result<SpotifydConfig, Error> {
    let mut config = SpotifydConfig::default();

    let config_file = config_path
        .or_else(|| {
            info!("Couldn't find config file, continuing with default configuration.");
            None
        })
        .and_then(|config_path| match Ini::load_from_file(config_path) {
            Ok(ini_file) => Some(ini_file),
            Err(err) => {
                info!(
                    "Couldn't read configuration file, continuing with default configuration: {}",
                    err
                );
                None
            }
        })
        .unwrap_or_else(|| {
            // Whenever we do not have a configuration file, we default to an empty one.
            ini::Ini::new()
        });

    let global = config_file.section(Some("global".to_owned()));
    let spotifyd = config_file.section(Some("spotifyd".to_owned()));

    let lookup = |field| {
        if matches.opt_defined(field) {
            if let Some(val) = matches.opt_str(field) {
                return Some(val);
            }
        }
        spotifyd
            .and_then(|s| s.get(field).map(String::clone))
            .or_else(|| global.and_then(|s| s.get(field).map(String::clone)))
    };

    update(
        &mut config.cache,
        lookup("cache_path")
            .map(PathBuf::from)
            .and_then(|p| Some(Cache::new(p, true)))
            .map(Some),
    );

    config.username = lookup("username");
    config.password = match lookup("password") {
        Some(password) => Some(password),
        None => match lookup("password_cmd") {
            Some(ref cmd) => match run_program(cmd) {
                Ok(s) => Some(s.trim().to_string()),
                Err(e) => return Err(Error::subprocess_with_err(cmd, e)),
            },
            None => None,
        },
    };
    if let Some(ref value) = lookup("use-keyring") {
        if value == "true" {
            config.use_keyring = true;
        }
    }
    config.backend = lookup("backend");
    config.audio_device = lookup("device");
    config.control_device = lookup("control");
    config.mixer = lookup("mixer");
    update(
        &mut config.volume_controller,
        lookup("volume-control").and_then(|s| VolumeController::from_str(&*s).ok()),
    );
    config.device_name = lookup("device_name").unwrap_or_else(|| {
        if let Some(h) = hostname::get_hostname() {
            format!("Spotifyd@{}", h)
        } else {
            "Spotifyd".to_string()
        }
    });
    config.onevent = lookup("onevent");
    config.player_config.normalisation = matches.opt_present("volume-normalisation")
        || spotifyd
            .and_then(|s| s.get("volume-normalisation").map(String::clone))
            .or_else(|| global.and_then(|g| g.get("volume-normalisation").map(String::clone)))
            .unwrap_or_else(|| "false".to_string())
            == "true";

    config.player_config.normalisation_pregain = match lookup("normalisation-pregain") {
        Some(db) => db
            .parse::<f32>()
            .map_err(|_| Error::from(ErrorKind::NormalisationPregainInvalid))?,
        None => PlayerConfig::default().normalisation_pregain,
    };

    update(
        &mut config.player_config.bitrate,
        lookup("bitrate").and_then(|s| Bitrate::from_str(&*s).ok()),
    );
    update(&mut config.session_config.device_id, lookup("device_name"));

    config.pid = lookup("pid");
    Ok(config)
}

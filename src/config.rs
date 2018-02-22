use std::error::Error;
use std::path::{Path, PathBuf};
use std::convert::From;
use std::fs::metadata;
use std::mem::swap;
use std::str::FromStr;

use librespot::playback::config::{Bitrate, PlayerConfig};
use librespot::core::config::SessionConfig;
use librespot::core::session::device_id;
use librespot::core::cache::Cache;
use librespot::core::version;

use xdg;
use ini::Ini;
use getopts::Matches;

use hostname;

const CONFIG_FILE: &str = "spotifyd.conf";

pub enum VolumeController {
    Alsa,
    SoftVol,
}

impl FromStr for VolumeController {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &*s.to_uppercase() {
            "ALSA" => Ok(VolumeController::Alsa),
            "SOFTVOL" => Ok(VolumeController::SoftVol),
            _ => Err(()),
        }
    }
}

pub struct SpotifydConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub cache: Option<Cache>,
    pub backend: Option<String>,
    pub audio_device: Option<String>,
    pub mixer: Option<String>,
    pub volume_controller: VolumeController,
    pub device_name: String,
    pub player_config: PlayerConfig,
    pub session_config: SessionConfig,
}

impl Default for SpotifydConfig {
    fn default() -> SpotifydConfig {
        SpotifydConfig {
            username: None,
            password: None,
            cache: None,
            backend: None,
            audio_device: None,
            mixer: None,
            volume_controller: VolumeController::SoftVol,
            device_name: "Spotifyd".to_string(),
            player_config: PlayerConfig {
                bitrate: Bitrate::Bitrate160,
                onstart: None,
                onstop: None,
            },
            session_config: SessionConfig {
                user_agent: version::version_string(),
                device_id: device_id("Spotifyd"),
            },
        }
    }
}

pub fn get_config_file() -> Result<PathBuf, Box<Error>> {
    let etc_conf = format!("/etc/{}", CONFIG_FILE);
    let xdg_dirs = try!(xdg::BaseDirectories::with_prefix("spotifyd"));
    xdg_dirs
        .find_config_file(CONFIG_FILE)
        .or_else(|| {
            metadata(&*etc_conf).ok().and_then(|meta| {
                if meta.is_file() {
                    Some(etc_conf.into())
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| From::from("Couldn't find a config file."))
}

fn update<T>(r: &mut T, val: Option<T>) {
    if let Some(mut v) = val {
        swap(r, &mut v);
    }
}

pub fn get_config<P: AsRef<Path>>(config_path: Option<P>, matches: &Matches) -> SpotifydConfig {
    let mut config = SpotifydConfig::default();

    let config_path = match config_path {
        Some(c) => c,
        None => {
            info!("Couldn't find config file, continuing with default configuration.");
            return config;
        }
    };

    let config_file = match Ini::load_from_file(config_path) {
        Ok(c) => c,
        Err(e) => {
            info!(
                "Couldn't read configuration file, continuing with default configuration: {}",
                e
            );
            return config;
        }
    };

    let global = config_file.section(Some("global".to_owned()));
    let spotifyd = config_file.section(Some("spotifyd".to_owned()));

    let lookup = |field| {
        matches
            .opt_str(field)
            .or_else(|| spotifyd.and_then(|s| s.get(field).map(String::clone))
                                .or_else(|| global.and_then(|s| s.get(field).map(String::clone))))
    };

    update(
        &mut config.cache,
        lookup("cache_path")
            .map(PathBuf::from)
            .and_then(|p| Some(Cache::new(p, true)))
            .map(Some),
    );

    config.username = lookup("username");
    config.password = lookup("password");
    config.backend = lookup("backend");
    config.audio_device = lookup("device");
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
    config.player_config.onstart = lookup("onstart");
    config.player_config.onstop = lookup("onstop");
    update(
        &mut config.player_config.bitrate,
        lookup("bitrate").and_then(|s| Bitrate::from_str(&*s).ok()),
    );
    update(&mut config.session_config.device_id, lookup("device_name"));

    config
}

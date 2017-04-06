use std::error::Error;
use std::path::{PathBuf, Path};
use std::convert::From;
use std::fs::metadata;
use std::mem::swap;
use std::str::FromStr;

use librespot::session::{Bitrate, Config as SessionConfig, device_id};
use librespot::cache::Cache;
use librespot::version;

use xdg;
use ini::Ini;

const CONFIG_FILE: &'static str = "spotifyd.conf";

pub struct SpotifydConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub cache: Option<Cache>,
    pub backend: Option<String>,
    pub device: Option<String>,
    pub session_config: SessionConfig,
}

impl Default for SpotifydConfig {
    fn default() -> SpotifydConfig {
        SpotifydConfig {
            username: None,
            password: None,
            cache: None,
            backend: None,
            device: None,
            session_config: SessionConfig {
                bitrate: Bitrate::Bitrate160,
                user_agent: version::version_string(),
                onstart: None,
                onstop: None,
                name: "Spotifyd".to_owned(),
                device_id: device_id("Spotifyd"),
            },
        }
    }
}

pub fn get_config_file() -> Result<PathBuf, Box<Error>> {
    let etc_conf = format!("/etc/{}", CONFIG_FILE);
    let xdg_dirs = try!(xdg::BaseDirectories::with_prefix("spotifyd"));
    xdg_dirs.find_config_file(CONFIG_FILE)
        .or_else(|| {
            metadata(&*etc_conf)
                .ok()
                .and_then(|meta| if meta.is_file() {
                    Some(etc_conf.into())
                } else {
                    None
                })
        })
        .ok_or(From::from("Couldn't find a config file."))
}

fn update<T>(r: &mut T, val: Option<T>) {
    if let Some(mut v) = val {
        swap(r, &mut v);
    }
}

pub fn get_config<P: AsRef<Path>>(config_path: Option<P>) -> SpotifydConfig {
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
            info!("Couldn't read configuration file, continuing with default configuration: {}",
                  e);
            return config;
        }
    };

    let global = config_file.section(Some("global".to_owned()));
    let spotifyd = config_file.section(Some("spotifyd".to_owned()));

    let lookup = |field| spotifyd.and_then(|s| s.get(field)).or(global.and_then(|s| s.get(field)));

    update(&mut config.cache,
           lookup("cache_path")
               .map(String::clone)
               .map(PathBuf::from)
               .and_then(|p| Some(Cache::new(p)))
               .map(|c| Some(c)));

    config.username = lookup("username").map(String::clone);
    config.password = lookup("password").map(String::clone);
    config.backend = lookup("backend").map(String::clone);
    config.device = lookup("device").map(String::clone);
    config.session_config.onstart = lookup("onstart").map(String::clone);
    config.session_config.onstop = lookup("onstop").map(String::clone);
    update(&mut config.session_config.bitrate,
           lookup("bitrate").map(String::as_ref).and_then(|s| Bitrate::from_str(s).ok()));
    update(&mut config.session_config.device_id,
           lookup("device_name").map(String::clone));

    return config;
}

use std::error::Error;
use std::path::PathBuf;
use std::convert::From;
use std::fs::metadata;

use librespot::session::{Bitrate, Config as SessionConfig};
use librespot::cache::{NoCache, Cache, DefaultCache};
use librespot::version;

use xdg;
use ini::Ini;

const CONFIG_FILE: &'static str = "spotifyd.conf";

pub struct SpotifydConfig {
    pub log_path: Option<PathBuf>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub cache: Box<Cache + Send + Sync>,
    pub backend: Option<String>,
    pub session_config: SessionConfig,
}

fn get_config_file() -> Result<PathBuf, Box<Error>> {
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

pub fn read_config() -> Result<SpotifydConfig, Box<Error>> {
    let conf_path = try!(get_config_file());
    let conf = try!(Ini::load_from_file(conf_path.to_str().unwrap()));

    let global = conf.section(Some("global".to_owned()));
    let spotifyd = conf.section(Some("spotifyd".to_owned()));

    let lookup = |field| spotifyd.and_then(|s| s.get(field)).or(global.and_then(|s| s.get(field)));

    let bitrate = spotifyd.and_then(|s| s.get("bitrate"))
        .and_then(|b| match b.trim() {
            "96" => Some(Bitrate::Bitrate96),
            "160" => Some(Bitrate::Bitrate160),
            "320" => Some(Bitrate::Bitrate320),
            _ => {
                error!("Invalid bitrate {}!", b.trim());
                None
            }
        })
        .unwrap_or(Bitrate::Bitrate160);

    let cache_path = lookup("cache_path").map(String::clone).map(PathBuf::from);
    let cache: Box<Cache + Send + Sync> = if let Some(p) = cache_path {
        if let Ok(c) = DefaultCache::new(p) {
            Box::new(c)
        } else {
            error!("Couldn't create cache, will continue without cache.");
            Box::new(NoCache)
        }
    } else {
        info!("No cache specified, will continue without cache.");
        Box::new(NoCache)
    };

    Ok(SpotifydConfig {
        log_path: lookup("log_path").map(|p| PathBuf::from(p)),
        username: lookup("username").map(String::clone),
        password: lookup("password").map(String::clone),
        backend: lookup("backend").map(String::clone),
        cache: cache,
        session_config: SessionConfig {
            bitrate: bitrate,
            user_agent: version::version_string(),
            onstart: lookup("onstart").map(String::clone),
            onstop: lookup("onstop").map(String::clone),
            device_name: lookup("device_name").map(String::clone).unwrap_or("Spotifyd".to_owned()),
        },
    })
}

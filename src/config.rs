use failure::{Error, Fail};

use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use clap::AppSettings;
use lazy_static::lazy_static;

use std::str::{FromStr};
use std::string::ToString;
use std::path::PathBuf;
use std::fmt;
use std::fs;
use std::error::Error as StdError;
use serde::de;

use app_dirs2::*;
use xdg;
use log::info;
use librespot::core::cache::Cache;
use librespot::core::version;
use librespot::playback::config::PlayerConfig;
use librespot::core::config::SessionConfig;
use librespot::playback::config::Bitrate as LSBitrate;

use crate::process::run_program;
use crate::utils;
use sha1::{Digest, Sha1};

const APP_INFO: AppInfo = AppInfo { name: "spotifyd", author: "various" };
const CONFIG_FILE_NAME: &str = "spotifyd.conf";

#[derive(Clone, Debug, Fail)]
pub enum ParseError {
    #[fail(display = "invalid backend: {}", name)]
    InvalidBackend {
        name: String,
    },
    #[fail(display = "invalid volume controller: {}", name)]
    InvalidVolumeController {
        name: String,
    },
    #[fail(display = "invalid bitrate: {}", name)]
    InvalidBitrate {
        name: String,
    },
}

lazy_static! {
    static ref BACKEND_VALUES: Vec<&'static str> = vec!["alsa", "pulseaudio", "portaudio"];
}

/// The backend used by librespot
#[derive(Clone, Copy, Debug, Deserialize, StructOpt)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Alsa,
    PortAudio,
    PulseAudio,
}

impl FromStr for Backend {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "alsa" => Ok(Backend::Alsa),
            "portaudio" => Ok(Backend::PortAudio),
            "pulseaudio" => Ok(Backend::PulseAudio),
            _ => unreachable!(),
        }
    }
}

impl ToString for Backend {
    fn to_string(&self) -> String {
        match self {
            Backend::Alsa => "alsa".into(),
            Backend::PortAudio => "portaudio".into(),
            Backend::PulseAudio => "pulseaudio".into(),
            _ => unreachable!(),
        }
    }
}

lazy_static! {
    static ref VOLUME_CONTROLLER_VALUES: Vec<&'static str> = vec!["alsa", "alsa_linear", "softvol"];
}

#[derive(Clone, Copy, Debug, Deserialize, StructOpt)]
#[serde(rename_all = "snake_case")]
pub enum VolumeController {
    Alsa,
    AlsaLinear,
    #[serde(rename = "softvol")]
    SoftVolume,
}

impl FromStr for VolumeController {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "alsa" => Ok(VolumeController::Alsa),
            "alsa_linear" => Ok(VolumeController::AlsaLinear),
            "softvol" => Ok(VolumeController::SoftVolume),
            _ => unreachable!(),
        }
    }
}

lazy_static! {
    static ref BITRATE_VALUES: Vec<&'static str> = vec!["96", "160", "320"];
}

/// Spotify's audio bitrate
#[derive(Clone, Copy, Debug, Deserialize, StructOpt)]
pub enum Bitrate {
    #[serde(rename = "96")]
    Bitrate96,
    #[serde(rename = "160")]
    Bitrate160,
    #[serde(rename = "320")]
    Bitrate320,
}

impl FromStr for Bitrate {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "96" => Ok(Bitrate::Bitrate96),
            "160" => Ok(Bitrate::Bitrate160),
            "320" => Ok(Bitrate::Bitrate320),
            _ => unreachable!(),
        }
    }
}

impl Into<LSBitrate> for Bitrate {
    fn into(self) -> LSBitrate {
        match self {
            Bitrate::Bitrate96 => LSBitrate::Bitrate96,
            Bitrate::Bitrate160 => LSBitrate::Bitrate160,
            Bitrate::Bitrate320 => LSBitrate::Bitrate320,
            _ => unreachable!(),
        }
    }
}

struct BoolFromStr;

impl<'de> de::Visitor<'de> for BoolFromStr {
    type Value = bool;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string containing \"true\" or \"false\"")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where E: serde::de::Error
    {
        println!("Trying to convert {}", s);
        println!("COnverted to {}", bool::from_str(s).unwrap());
        bool::from_str(s).map_err(serde::de::Error::custom)
    }
}

fn de_from_str<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where D: de::Deserializer<'de>
{
    deserializer.deserialize_str(BoolFromStr)
}

#[derive(Debug, Default, StructOpt)]
#[structopt(
    about = "A Spotify daemon",
    author,
    name = "spotifyd",
    setting(AppSettings::ColoredHelp)
)]
pub struct CliConfig {
    /// The path to the config file to use
    #[structopt(long, value_name = "string")]
    pub config_path: Option<PathBuf>,

    /// If set, starts spotifyd as a unix daemon
    #[structopt(long)]
    pub daemon: bool,

    /// Prints more verbose output
    #[structopt(long)]
    pub verbose: bool,

    /// Process id to launch the daemon on
    #[structopt(long)]
    pub pid: Option<i32>,

    #[structopt(flatten)]
    pub file_config: FileConfig,
}

#[derive(Default, Deserialize, StructOpt)]
pub struct FileConfig {
    /// The Spotify account user name
    #[structopt(long, short, value_name = "string")]
    username: Option<String>,

    /// The Spotify account password
    #[structopt(conflicts_with = "password_cmd", long, short, value_name = "string")]
    password: Option<String>,

    /// Enables keyring password access
    #[structopt(long)]
    #[serde(alias = "use-keyring", deserialize_with = "de_from_str")]
    use_keyring: bool,

    /// A command that can be used to retrieve the Spotify account password
    #[structopt(conflicts_with = "password", long, short = "P", value_name = "string", visible_alias = "password_cmd")]
    password_cmd: Option<String>,

    /// A script that gets evaluated in the user's shell when the song changes
    #[structopt(visible_alias = "onevent", long, value_name = "string")]
    #[serde(alias = "onevent")]
    on_song_change_hook: Option<String>,

    /// The cache path used to store credentials and music file artifacts
    #[structopt(long, parse(from_os_str), short, value_name = "string")]
    cache_path: Option<PathBuf>,

    /// The audio backend to use
    #[structopt(long, short, possible_values = &BACKEND_VALUES, value_name = "string")]
    backend: Option<Backend>,

    /// The volume controller to use
    #[structopt(long, short, possible_values = &VOLUME_CONTROLLER_VALUES, visible_alias = "volume-control")]
    #[serde(alias = "volume-control")]
    volume_controller: Option<VolumeController>,

    /// The audio device
    #[structopt(long, value_name = "string")]
    device: Option<String>,

    /// The control device
    #[structopt(long, value_name = "string")]
    control: Option<String>,

    /// The mixer to use
    #[structopt(long, value_name = "string")]
    mixer: Option<String>,

    /// The device name displayed in Spotify
    #[structopt(long, short, value_name = "string")]
    device_name: Option<String>,

    /// The bitrate of the streamed audio data
    #[structopt(long, short = "B", possible_values = &BITRATE_VALUES, value_name = "number")]
    bitrate: Option<Bitrate>,

    /// Enable to normalize the volume during playback
    #[structopt(long)]
    #[serde(default, deserialize_with = "de_from_str")]
    volume_normalisation: bool,

    /// A custom pregain applied before sending the audio to the output device
    #[structopt(long, value_name = "number")]
    normalisation_pregain: Option<f32>,

    /// The port used for the Spotify Connect discovery
    #[structopt(long)]
    zeroconf_port: Option<u16>,
}

impl fmt::Debug for FileConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let password_value = if self.password.is_some() {
            Some("taken out for privacy")
        } else {
            None
        };

        f.debug_struct("FileConfig")
            .field("username", &self.username)
            .field("password", &password_value)
            .field("password_cmd", &self.password_cmd)
            .field("use_keyring", &self.use_keyring)
            .field("on_change_song_hook", &self.on_song_change_hook)
            .field("cache_path", &self.cache_path)
            .field("backend", &self.backend)
            .field("volume_controller", &self.volume_controller)
            .field("device", &self.device)
            .field("control", &self.control)
            .field("mixer", &self.mixer)
            .field("device_name", &self.device_name)
            .field("bitrate", &self.bitrate)
            .field("volume_normalisation", &self.volume_normalisation)
            .field("normalisation_pregain", &self.normalisation_pregain)
            .field("zeroconf_port", &self.zeroconf_port)
            .finish()
    }
}

impl CliConfig {
    pub fn load_config_file_values(&mut self) {
        let config_file_path = self.config_path.clone()
            .or_else(get_config_file);

        if config_file_path.is_none() {
            info!("No config file specified. Running with default values");
            return;
        }
        let unwrapped_config_file_path = config_file_path.unwrap();
        info!("Loading config from {:?}", &unwrapped_config_file_path);

        let config_file = fs::File::open(&unwrapped_config_file_path);
        if config_file.is_err() {
            info!("Failed to open config file at {:?}", &unwrapped_config_file_path);
            return;
        }

        let bufreader = std::io::BufReader::new(config_file.unwrap());
        let config_content: FileConfig = serde_ini::from_bufread(bufreader).unwrap();

        self.file_config.merge_with(config_content);
    }
}

impl FileConfig {
    pub fn merge_with(&mut self, other: FileConfig) {
        macro_rules! merge {
            ($($x:ident),+) => {
                $(self.$x = self.$x.clone().or_else(|| other.$x.clone());)+
            }
        }

        // Handles Option<T> merging.
        merge!(backend, username, password, password_cmd, normalisation_pregain, bitrate,
            device_name, mixer, control, device, volume_controller, cache_path,
            on_song_change_hook);
        
        // Handles boolean merging.
        self.use_keyring = self.use_keyring | other.use_keyring;
        self.volume_normalisation = self.volume_normalisation | other.volume_normalisation;
    }
}

pub(crate) fn get_config_file() -> Option<PathBuf> {
    let etc_conf = format!("/etc/{}", CONFIG_FILE_NAME);
    let xdg_dirs = xdg::BaseDirectories::with_prefix("spotifyd").ok()?;
    xdg_dirs.find_config_file(CONFIG_FILE_NAME).or_else(|| {
        fs::metadata(&*etc_conf).ok().and_then(|meta| {
            if meta.is_file() {
                Some(etc_conf.into())
            } else {
                None
            }
        })
    })
}

fn device_id(name: &str) -> String {
    hex::encode(&Sha1::digest(name.as_bytes()))
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
    pub(crate) shell: String,
    pub(crate) zeroconf_port: Option<u16>,
}

pub(crate) fn get_internal_config(config: CliConfig) -> SpotifydConfig {
    let cache = config.file_config.cache_path
        .map(PathBuf::from)
        .and_then(|path| Some(Cache::new(path, true)));

    let bitrate: LSBitrate = config.file_config.bitrate
        .unwrap_or(Bitrate::Bitrate160)
        .into();

    let backend = config.file_config.backend
        .unwrap_or(Backend::Alsa).to_string();

    let device_name = config.file_config.device_name
        .unwrap_or("Spotifyd".to_string());

    let normalisation_pregain = config.file_config.normalisation_pregain
        .unwrap_or(0.0f32);

    let pid = config.pid
        .and_then(|f| Some(f.to_string()))
        .or_else(|| None);

    SpotifydConfig {
        username: config.file_config.username,
        password: config.file_config.password,
        use_keyring: config.file_config.use_keyring,
        cache,
        backend: Some(backend),
        audio_device: config.file_config.device,
        control_device: config.file_config.control,
        mixer: config.file_config.mixer,
        volume_controller: config.file_config.volume_controller.unwrap(),
        device_name,
        player_config: PlayerConfig {
            bitrate,
            normalisation: config.file_config.volume_normalisation,
            normalisation_pregain,
        },
        session_config: SessionConfig {
            user_agent: version::version_string(),
            device_id: device_id("Spotifyd"),
            proxy: None,
            ap_port: Some(443),
        },
        onevent: config.file_config.on_song_change_hook,
        pid,
        shell: utils::get_shell().unwrap_or_else(|| {
            info!("Unable to identify shell. Defaulting to \"sh\".");
            "sh".to_string()
        }),
        zeroconf_port: config.file_config.zeroconf_port,
    }
}

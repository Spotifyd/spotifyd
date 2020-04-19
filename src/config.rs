use gethostname::gethostname;
use lazy_static::lazy_static;
use librespot::{
    core::{cache::Cache, config::DeviceType as LSDeviceType, config::SessionConfig, version},
    playback::config::{Bitrate as LSBitrate, PlayerConfig},
};
use log::{error, info};
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use sha1::{Digest, Sha1};
use structopt::{clap::AppSettings, StructOpt};
use url::Url;
use xdg;

use std::{fmt, fs, path::PathBuf, str::FromStr, string::ToString};

use crate::{
    error::{Error as CrateError, ParseError},
    process::run_program,
    utils,
};

const CONFIG_FILE_NAME: &str = "spotifyd.conf";

lazy_static! {
    static ref BACKEND_VALUES: Vec<&'static str> = {
        let mut vec = Vec::new();

        if cfg!(feature = "alsa_backend") {
            vec.push("alsa");
        }
        if cfg!(feature = "pulseaudio_backend") {
            vec.push("pulseaudio");
        }
        if cfg!(feature = "portaudio_backend") {
            vec.push("portaudio");
        }
        if cfg!(feature = "rodio_backend") {
            vec.push("rodio");
        }

        vec
    };
}

/// The backend used by librespot
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, StructOpt)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Alsa,
    PortAudio,
    PulseAudio,
    Rodio,
}

impl FromStr for Backend {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "alsa" => Ok(Backend::Alsa),
            "portaudio" => Ok(Backend::PortAudio),
            "pulseaudio" => Ok(Backend::PulseAudio),
            "rodio" => Ok(Backend::Rodio),
            _ => unreachable!(),
        }
    }
}

impl ToString for Backend {
    fn to_string(&self) -> String {
        match self {
            Backend::Alsa => "alsa".to_string(),
            Backend::PortAudio => "portaudio".to_string(),
            Backend::PulseAudio => "pulseaudio".to_string(),
            Backend::Rodio => "rodio".to_string(),
        }
    }
}

lazy_static! {
    static ref VOLUME_CONTROLLER_VALUES: Vec<&'static str> = {
        let mut vec = vec!["softvol"];

        if cfg!(feature = "alsa_backend") {
            vec.push("alsa");
            vec.push("alsa_linear");
        }

        vec
    };
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, StructOpt)]
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
    static ref DEVICETYPE_VALUES: Vec<&'static str> = vec![
        "computer",
        "tablet",
        "smartphone",
        "speaker",
        "tv",
        "avr",
        "stb",
        "audiodongle"
    ];
}

// Spotify's device type (copied from it's config.rs)
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, StructOpt)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Unknown = 0,
    Computer = 1,
    Tablet = 2,
    Smartphone = 3,
    Speaker = 4,
    TV = 5,
    AVR = 6,
    STB = 7,
    AudioDongle = 8,
}

impl From<LSDeviceType> for DeviceType {
    fn from(item: LSDeviceType) -> Self {
        match item {
            LSDeviceType::Unknown => DeviceType::Unknown,
            LSDeviceType::Computer => DeviceType::Computer,
            LSDeviceType::Tablet => DeviceType::Tablet,
            LSDeviceType::Smartphone => DeviceType::Smartphone,
            LSDeviceType::Speaker => DeviceType::Speaker,
            LSDeviceType::TV => DeviceType::TV,
            LSDeviceType::AVR => DeviceType::AVR,
            LSDeviceType::STB => DeviceType::STB,
            LSDeviceType::AudioDongle => DeviceType::AudioDongle,
        }
    }
}

impl From<&DeviceType> for LSDeviceType {
    fn from(item: &DeviceType) -> Self {
        match item {
            DeviceType::Unknown => LSDeviceType::Unknown,
            DeviceType::Computer => LSDeviceType::Computer,
            DeviceType::Tablet => LSDeviceType::Tablet,
            DeviceType::Smartphone => LSDeviceType::Smartphone,
            DeviceType::Speaker => LSDeviceType::Speaker,
            DeviceType::TV => LSDeviceType::TV,
            DeviceType::AVR => LSDeviceType::AVR,
            DeviceType::STB => LSDeviceType::STB,
            DeviceType::AudioDongle => LSDeviceType::AudioDongle,
        }
    }
}

impl FromStr for DeviceType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let dt = LSDeviceType::from_str(s).unwrap();
        Ok(dt.into())
    }
}

impl ToString for DeviceType {
    fn to_string(&self) -> String {
        let dt: LSDeviceType = self.into();
        format!("{}", dt)
    }
}

lazy_static! {
    static ref BITRATE_VALUES: Vec<&'static str> = vec!["96", "160", "320"];
}

/// Spotify's audio bitrate
#[derive(Clone, Copy, Debug, Deserialize_repr, PartialEq, StructOpt)]
#[repr(u16)]
pub enum Bitrate {
    Bitrate96 = 96,
    Bitrate160 = 160,
    Bitrate320 = 320,
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
        }
    }
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

    /// If set, starts spotifyd without detaching
    #[structopt(long)]
    pub no_daemon: bool,

    /// Prints more verbose output
    #[structopt(long)]
    pub verbose: bool,

    /// Path to PID file.
    #[structopt(long)]
    pub pid: Option<PathBuf>,

    #[structopt(flatten)]
    pub shared_config: SharedConfigValues,
}

// A struct that holds all allowed config fields.
// The actual config file is made up of two sections, spotifyd and global.
#[derive(Clone, Default, Deserialize, PartialEq, StructOpt)]
pub struct SharedConfigValues {
    /// The Spotify account user name
    #[structopt(long, short, value_name = "string")]
    username: Option<String>,

    /// The Spotify account password
    #[structopt(conflicts_with = "password_cmd", long, short, value_name = "string")]
    password: Option<String>,

    /// Enables keyring password access
    #[cfg_attr(
        feature = "dbus_keyring",
        structopt(long),
        serde(alias = "use-keyring", default)
    )]
    #[cfg_attr(not(feature = "dbus_keyring"), structopt(skip), serde(skip))]
    use_keyring: bool,

    /// A command that can be used to retrieve the Spotify account password
    #[structopt(
        conflicts_with = "password",
        long,
        short = "P",
        value_name = "string",
        visible_alias = "password_cmd"
    )]
    password_cmd: Option<String>,

    /// A script that gets evaluated in the user's shell when the song changes
    #[structopt(visible_alias = "onevent", long, value_name = "string")]
    #[serde(alias = "onevent")]
    on_song_change_hook: Option<String>,

    /// The cache path used to store credentials and music file artifacts
    #[structopt(long, parse(from_os_str), short, value_name = "string")]
    cache_path: Option<PathBuf>,

    /// Disable the use of audio cache
    #[structopt(long)]
    #[serde(default)]
    no_audio_cache: bool,

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
    #[serde(default)]
    volume_normalisation: bool,

    /// A custom pregain applied before sending the audio to the output device
    #[structopt(long, value_name = "number")]
    normalisation_pregain: Option<f32>,

    /// The port used for the Spotify Connect discovery
    #[structopt(long, value_name = "number")]
    zeroconf_port: Option<u16>,

    /// The proxy used to connect to spotify's servers
    #[structopt(long, value_name = "string")]
    proxy: Option<String>,

    /// The device type shown to clients
    #[structopt(long, possible_values = &DEVICETYPE_VALUES, value_name = "string")]
    device_type: Option<DeviceType>,

    /// Autoplay on connect
    #[structopt(long)]
    #[serde(default)]
    autoplay: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct FileConfig {
    global: Option<SharedConfigValues>,
    spotifyd: Option<SharedConfigValues>,
}

impl FileConfig {
    pub fn get_merged_sections(self) -> Option<SharedConfigValues> {
        let global_config_section = self.global;
        let spotifyd_config_section = self.spotifyd;

        let merged_config: Option<SharedConfigValues>;
        // First merge the two sections together. The spotifyd has priority over global
        // section.
        if let Some(mut spotifyd_section) = spotifyd_config_section {
            // spotifyd section exists. Try to merge it with global section.
            if let Some(global_section) = global_config_section {
                spotifyd_section.merge_with(global_section);
                merged_config = Some(spotifyd_section);
            } else {
                // There is no global section. Just use the spotifyd section.
                merged_config = Some(spotifyd_section);
            }
        } else {
            // No spotifyd config available. Check for global and use that, if both are
            // none, use none.
            merged_config = global_config_section;
        }

        merged_config
    }
}

impl fmt::Debug for SharedConfigValues {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let placeholder = "taken out for privacy";

        // TODO: somehow replace with a appropiate macro.
        let password_value = if self.password.is_some() {
            Some(&placeholder)
        } else {
            None
        };

        let password_cmd_value = if self.password_cmd.is_some() {
            Some(&placeholder)
        } else {
            None
        };

        let username_value = if self.username.is_some() {
            Some(&placeholder)
        } else {
            None
        };

        f.debug_struct("SharedConfigValues")
            .field("username", &username_value)
            .field("password", &password_value)
            .field("password_cmd", &password_cmd_value)
            .field("use_keyring", &self.use_keyring)
            .field("on_song_change_hook", &self.on_song_change_hook)
            .field("cache_path", &self.cache_path)
            .field("no-audio-cache", &self.no_audio_cache)
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
            .field("proxy", &self.proxy)
            .field("device_type", &self.device_type)
            .finish()
    }
}

impl CliConfig {
    pub fn load_config_file_values(&mut self) {
        let config_file_path = match self.config_path.clone().or_else(get_config_file) {
            Some(p) => p,
            None => {
                info!("No config file specified. Running with default values");
                return;
            }
        };
        info!("Loading config from {:?}", &config_file_path);

        let content = match std::fs::read_to_string(config_file_path) {
            Ok(s) => s,
            Err(e) => {
                info!("Failed reading config file: {}", e);
                return;
            }
        };

        let config_content: FileConfig = toml::from_str(&content).unwrap();

        // The call to get_merged_sections consumes the FileConfig!
        if let Some(merged_sections) = config_content.get_merged_sections() {
            self.shared_config.merge_with(merged_sections);
        }
    }
}

impl SharedConfigValues {
    pub fn merge_with(&mut self, other: SharedConfigValues) {
        macro_rules! merge {
            ($($x:ident),+) => {
                $(self.$x = self.$x.clone().or_else(|| other.$x.clone());)+
            }
        }

        // Handles Option<T> merging.
        merge!(
            backend,
            username,
            password,
            password_cmd,
            normalisation_pregain,
            bitrate,
            device_name,
            mixer,
            control,
            device,
            volume_controller,
            cache_path,
            on_song_change_hook,
            zeroconf_port,
            proxy,
            device_type
        );

        // Handles boolean merging.
        self.use_keyring |= other.use_keyring;
        self.volume_normalisation |= other.volume_normalisation;
        self.no_audio_cache |= other.no_audio_cache;
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
    #[allow(unused)]
    pub(crate) use_keyring: bool,
    pub(crate) cache: Option<Cache>,
    pub(crate) backend: Option<String>,
    pub(crate) audio_device: Option<String>,
    #[allow(unused)]
    pub(crate) control_device: Option<String>,
    #[allow(unused)]
    pub(crate) mixer: Option<String>,
    #[allow(unused)]
    pub(crate) volume_controller: VolumeController,
    pub(crate) device_name: String,
    pub(crate) player_config: PlayerConfig,
    pub(crate) session_config: SessionConfig,
    pub(crate) onevent: Option<String>,
    pub(crate) pid: Option<String>,
    pub(crate) shell: String,
    pub(crate) zeroconf_port: Option<u16>,
    pub(crate) device_type: String,
    pub(crate) autoplay: bool,
}

pub(crate) fn get_internal_config(config: CliConfig) -> SpotifydConfig {
    let audio_cache = !config.shared_config.no_audio_cache;

    let cache = config
        .shared_config
        .cache_path
        .map(PathBuf::from)
        .map(|path| Cache::new(path, audio_cache));

    let bitrate: LSBitrate = config
        .shared_config
        .bitrate
        .unwrap_or(Bitrate::Bitrate160)
        .into();

    let backend = config
        .shared_config
        .backend
        .unwrap_or(Backend::Alsa)
        .to_string();

    let volume_controller = config
        .shared_config
        .volume_controller
        .unwrap_or(VolumeController::SoftVolume);

    let device_name = config
        .shared_config
        .device_name
        .filter(|s| !s.trim().is_empty())
        .filter(|s| !s.chars().any(char::is_whitespace))
        .unwrap_or_else(|| format!("{}@{}", "Spotifyd", gethostname().to_string_lossy()));

    let device_id = device_id(&device_name);

    let normalisation_pregain = config.shared_config.normalisation_pregain.unwrap_or(0.0f32);

    let autoplay = config.shared_config.autoplay;

    let device_type = config
        .shared_config
        .device_type
        .unwrap_or(DeviceType::Speaker)
        .to_string();

    let pid = config
        .pid
        .map(|f| {
            f.into_os_string()
                .into_string()
                .expect("Failed to convert PID file path to valid Unicode")
        })
        .or_else(|| None);

    let shell = utils::get_shell().unwrap_or_else(|| {
        info!("Unable to identify shell. Defaulting to \"sh\".");
        "sh".to_string()
    });

    let mut password = config.shared_config.password;
    if password.is_none() && config.shared_config.password_cmd.is_some() {
        info!("No password specified. Checking password_cmd");

        match config.shared_config.password_cmd {
            Some(ref cmd) => match run_program(&shell, cmd) {
                Ok(s) => password = Some(s.trim().to_string()),
                Err(e) => error!("{}", CrateError::subprocess_with_err(&shell, cmd, e)),
            },
            None => info!("No password_cmd specified"),
        }
    }
    let mut proxy_url = None;
    match config.shared_config.proxy {
        Some(s) => match Url::parse(&s) {
            Ok(url) => {
                if url.scheme() != "http" {
                    error!("Only HTTP proxies are supported!");
                } else {
                    proxy_url = Some(url);
                }
            }
            Err(err) => error!("Invalid proxy URL: {}", err),
        },
        None => info!("No proxy specified"),
    }
    SpotifydConfig {
        username: config.shared_config.username,
        password,
        use_keyring: config.shared_config.use_keyring,
        cache,
        backend: Some(backend),
        audio_device: config.shared_config.device,
        control_device: config.shared_config.control,
        mixer: config.shared_config.mixer,
        volume_controller,
        device_name,
        player_config: PlayerConfig {
            bitrate,
            normalisation: config.shared_config.volume_normalisation,
            normalisation_pregain,
        },
        session_config: SessionConfig {
            user_agent: version::version_string(),
            device_id,
            proxy: proxy_url,
            ap_port: Some(443),
        },
        onevent: config.shared_config.on_song_change_hook,
        pid,
        shell,
        zeroconf_port: config.shared_config.zeroconf_port,
        device_type,
        autoplay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_merging() {
        let mut spotifyd_section = SharedConfigValues::default();
        spotifyd_section.password = Some("123456".to_string());

        let mut global_section = SharedConfigValues::default();
        global_section.username = Some("testUserName".to_string());

        // The test only makes sense if both sections differ.
        assert!(spotifyd_section != global_section, true);

        let file_config = FileConfig {
            global: Some(global_section),
            spotifyd: Some(spotifyd_section.clone()),
        };
        let merged_config = file_config.get_merged_sections().unwrap();

        // Add the new field to spotifyd section.
        spotifyd_section.username = Some("testUserName".to_string());
        assert_eq!(merged_config, spotifyd_section);
    }
}

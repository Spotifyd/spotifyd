use crate::{
    error::{Error as CrateError, ParseError},
    process::run_program,
    utils,
};
use color_eyre::Report;
use gethostname::gethostname;
use librespot_core::{
    cache::Cache, config::DeviceType as LSDeviceType, config::SessionConfig, version,
};
use librespot_playback::config::{
    AudioFormat as LSAudioFormat, Bitrate as LSBitrate, PlayerConfig,
};
use log::{error, info, warn};
use serde::{de::Error, de::Unexpected, Deserialize, Deserializer};
use sha1::{Digest, Sha1};
use std::{fmt, fs, path::PathBuf, str::FromStr, string::ToString};
use structopt::{clap::AppSettings, StructOpt};
use url::Url;

const CONFIG_FILE_NAME: &str = "spotifyd.conf";

#[cfg(not(any(
    feature = "pulseaudio_backend",
    feature = "portaudio_backend",
    feature = "alsa_backend",
    feature = "rodio_backend"
)))]
compile_error!("At least one of the backend features is required!");
static BACKEND_VALUES: &[&str] = &[
    #[cfg(feature = "alsa_backend")]
    "alsa",
    #[cfg(feature = "pulseaudio_backend")]
    "pulseaudio",
    #[cfg(feature = "portaudio_backend")]
    "portaudio",
    #[cfg(feature = "rodio_backend")]
    "rodio",
];

/// The backend used by librespot
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, StructOpt)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Alsa,
    PortAudio,
    PulseAudio,
    Rodio,
}

fn default_backend() -> Backend {
    return Backend::from_str(BACKEND_VALUES.first().unwrap()).unwrap();
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

static VOLUME_CONTROLLER_VALUES: &[&str] = &[
    "softvol",
    #[cfg(feature = "alsa_backend")]
    "alsa",
    #[cfg(feature = "alsa_backend")]
    "alsa_linear",
    "none",
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, StructOpt)]
#[serde(rename_all = "snake_case")]
pub enum VolumeController {
    Alsa,
    AlsaLinear,
    #[serde(rename = "softvol")]
    SoftVolume,
    None,
}

impl FromStr for VolumeController {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "alsa" => Ok(VolumeController::Alsa),
            "alsa_linear" => Ok(VolumeController::AlsaLinear),
            "softvol" => Ok(VolumeController::SoftVolume),
            "none" => Ok(VolumeController::None),
            _ => unreachable!(),
        }
    }
}

static DEVICETYPE_VALUES: &[&str] = &[
    "computer",
    "tablet",
    "smartphone",
    "speaker",
    "tv",
    "avr",
    "stb",
    "audiodongle",
];

// Spotify's device type (copied from it's config.rs)
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, StructOpt)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Unknown = 0,
    Computer = 1,
    Tablet = 2,
    Smartphone = 3,
    Speaker = 4,
    #[serde(rename = "t_v")]
    Tv = 5,
    #[serde(rename = "a_v_r")]
    Avr = 6,
    #[serde(rename = "s_t_b")]
    Stb = 7,
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
            LSDeviceType::Tv => DeviceType::Tv,
            LSDeviceType::Avr => DeviceType::Avr,
            LSDeviceType::Stb => DeviceType::Stb,
            LSDeviceType::AudioDongle => DeviceType::AudioDongle,
            // TODO: Implement new LibreSpot device types in Spotifyd
            _ => DeviceType::Unknown,
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
            DeviceType::Tv => LSDeviceType::Tv,
            DeviceType::Avr => LSDeviceType::Avr,
            DeviceType::Stb => LSDeviceType::Stb,
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

static BITRATE_VALUES: &[&str] = &["96", "160", "320"];

/// Spotify's audio bitrate
#[derive(Clone, Copy, Debug, PartialEq, Eq, StructOpt)]
pub enum Bitrate {
    Bitrate96,
    Bitrate160,
    Bitrate320,
}

impl<'de> Deserialize<'de> for Bitrate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u16::deserialize(deserializer) {
            Ok(96) => Ok(Self::Bitrate96),
            Ok(160) => Ok(Self::Bitrate160),
            Ok(320) => Ok(Self::Bitrate320),
            Ok(x) => Err(D::Error::invalid_value(
                Unexpected::Unsigned(x.into()),
                &"a bitrate: 96, 160, 320",
            )),
            Err(e) => Err(e),
        }
    }
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

impl From<Bitrate> for LSBitrate {
    fn from(bitrate: Bitrate) -> Self {
        match bitrate {
            Bitrate::Bitrate96 => LSBitrate::Bitrate96,
            Bitrate::Bitrate160 => LSBitrate::Bitrate160,
            Bitrate::Bitrate320 => LSBitrate::Bitrate320,
        }
    }
}

#[cfg(feature = "dbus_mpris")]
static DBUSTYPE_VALUES: &[&str] = &["session", "system"];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, StructOpt)]
#[serde(rename_all = "snake_case")]
pub enum DBusType {
    Session,
    System,
}

impl FromStr for DBusType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "session" => Ok(DBusType::Session),
            "system" => Ok(DBusType::System),
            _ => unreachable!(),
        }
    }
}

impl ToString for DBusType {
    fn to_string(&self) -> String {
        match self {
            DBusType::Session => "session".to_string(),
            DBusType::System => "system".to_string(),
        }
    }
}

/// LibreSpot supported audio formats
static AUDIO_FORMAT_VALUES: &[&str] = &["F32", "S32", "S24", "S24_3", "S16"];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, StructOpt)]
pub enum AudioFormat {
    F32,
    S32,
    S24,
    S24_3,
    S16,
}

impl FromStr for AudioFormat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "F32" => Ok(AudioFormat::F32),
            "S32" => Ok(AudioFormat::S32),
            "S24" => Ok(AudioFormat::S24),
            "S24_3" => Ok(AudioFormat::S24_3),
            "S16" => Ok(AudioFormat::S16),
            _ => unreachable!(),
        }
    }
}

impl ToString for AudioFormat {
    fn to_string(&self) -> String {
        match self {
            AudioFormat::F32 => "F32".to_string(),
            AudioFormat::S32 => "S32".to_string(),
            AudioFormat::S24 => "S24".to_string(),
            AudioFormat::S24_3 => "S24_3".to_string(),
            AudioFormat::S16 => "S16".to_string(),
        }
    }
}

impl From<AudioFormat> for LSAudioFormat {
    fn from(audio_format: AudioFormat) -> Self {
        match audio_format {
            AudioFormat::F32 => LSAudioFormat::F32,
            AudioFormat::S32 => LSAudioFormat::S32,
            AudioFormat::S24 => LSAudioFormat::S24,
            AudioFormat::S24_3 => LSAudioFormat::S24_3,
            AudioFormat::S16 => LSAudioFormat::S16,
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
    #[structopt(conflicts_with = "username_cmd", long, short, value_name = "string")]
    username: Option<String>,

    /// A command that can be used to retrieve the Spotify account username
    #[structopt(
        conflicts_with = "username",
        long,
        short = "U",
        value_name = "string",
        visible_alias = "username_cmd"
    )]
    username_cmd: Option<String>,

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

    /// Enables the MPRIS interface
    #[cfg_attr(
        feature = "dbus_mpris",
        structopt(long),
        serde(alias = "use-mpris", default)
    )]
    #[cfg_attr(not(feature = "dbus_mpris"), structopt(skip), serde(skip))]
    use_mpris: Option<bool>,

    /// The Bus-type to use for the MPRIS interface
    #[cfg_attr(
        feature = "dbus_mpris",
        structopt(long, possible_values = &DBUSTYPE_VALUES, value_name = "string")
    )]
    #[cfg_attr(not(feature = "dbus_mpris"), structopt(skip), serde(skip))]
    dbus_type: Option<DBusType>,

    /// A command that can be used to retrieve the Spotify account password
    #[structopt(
        conflicts_with = "password",
        long,
        short = "P",
        value_name = "string",
        visible_alias = "password_cmd"
    )]
    password_cmd: Option<String>,

    /// Whether the credentials should be debugged.
    #[structopt(long)]
    #[serde(skip)]
    debug_credentials: bool,

    /// A script that gets evaluated in the user's shell when the song changes
    #[structopt(visible_alias = "onevent", long, value_name = "string")]
    #[serde(alias = "onevent")]
    on_song_change_hook: Option<String>,

    /// The cache path used to store credentials and music file artifacts
    #[structopt(long, parse(from_os_str), short, value_name = "string")]
    cache_path: Option<PathBuf>,

    /// The maximal cache size in bytes
    #[structopt(long)]
    max_cache_size: Option<u64>,

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

    /// The audio format of the streamed audio data
    #[structopt(long, possible_values = &AUDIO_FORMAT_VALUES, value_name = "string")]
    audio_format: Option<AudioFormat>,

    /// Initial volume between 0 and 100
    #[structopt(long, value_name = "initial_volume")]
    initial_volume: Option<String>,

    /// Enable to normalize the volume during playback
    #[structopt(long)]
    #[serde(default)]
    volume_normalisation: bool,

    /// A custom pregain applied before sending the audio to the output device
    #[structopt(long, value_name = "number")]
    normalisation_pregain: Option<f64>,

    /// The port used for the Spotify Connect discovery
    #[structopt(long, value_name = "number")]
    zeroconf_port: Option<u16>,

    /// The proxy used to connect to spotify's servers
    #[structopt(long, value_name = "string")]
    proxy: Option<String>,

    /// The device type shown to clients
    #[structopt(long, possible_values = &DEVICETYPE_VALUES, value_name = "string")]
    device_type: Option<DeviceType>,

    /// Start playing similar songs after your music has ended
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
            #[allow(clippy::branches_sharing_code)]
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

        macro_rules! extract_credential {
            ( $e:expr ) => {
                match $e {
                    Some(s) => match self.debug_credentials {
                        true => Some(s.as_str()),
                        false => Some(placeholder),
                    },
                    None => None,
                }
            };
        }

        let password_value = extract_credential!(&self.password);

        let password_cmd_value = extract_credential!(&self.password_cmd);

        let username_value = extract_credential!(&self.username);

        let username_cmd_value = extract_credential!(&self.username_cmd);

        f.debug_struct("SharedConfigValues")
            .field("username", &username_value)
            .field("username_cmd", &username_cmd_value)
            .field("password", &password_value)
            .field("password_cmd", &password_cmd_value)
            .field("use_keyring", &self.use_keyring)
            .field("use_mpris", &self.use_mpris)
            .field("dbus_type", &self.dbus_type)
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
            .field("audio_format", &self.audio_format)
            .field("initial_volume", &self.initial_volume)
            .field("volume_normalisation", &self.volume_normalisation)
            .field("normalisation_pregain", &self.normalisation_pregain)
            .field("zeroconf_port", &self.zeroconf_port)
            .field("proxy", &self.proxy)
            .field("device_type", &self.device_type)
            .field("autoplay", &self.autoplay)
            .field("max_cache_size", &self.max_cache_size)
            .finish()
    }
}

impl CliConfig {
    pub fn load_config_file_values(&mut self) -> Result<(), Report> {
        let config_file_path = match self.config_path.clone().or_else(get_config_file) {
            Some(p) => p,
            None => {
                info!("No config file specified. Running with default values");
                return Ok(());
            }
        };
        info!("Loading config from {:?}", &config_file_path);

        let content = match fs::read_to_string(config_file_path) {
            Ok(s) => s,
            Err(e) => {
                info!("Failed reading config file: {}", e);
                return Ok(());
            }
        };

        let config_content: FileConfig = toml::from_str(&content)?;

        // The call to get_merged_sections consumes the FileConfig!
        if let Some(merged_sections) = config_content.get_merged_sections() {
            self.shared_config.merge_with(merged_sections);
        }

        Ok(())
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
            username_cmd,
            password,
            password_cmd,
            normalisation_pregain,
            bitrate,
            initial_volume,
            device_name,
            mixer,
            control,
            device,
            volume_controller,
            cache_path,
            on_song_change_hook,
            zeroconf_port,
            proxy,
            device_type,
            use_mpris,
            max_cache_size,
            dbus_type,
            audio_format
        );

        // Handles boolean merging.
        self.use_keyring |= other.use_keyring;
        self.volume_normalisation |= other.volume_normalisation;
        self.no_audio_cache |= other.no_audio_cache;
        self.autoplay |= other.autoplay;
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn get_config_file() -> Option<PathBuf> {
    let etc_conf = format!("/etc/{}", CONFIG_FILE_NAME);
    let dirs = directories::BaseDirs::new()?;
    let mut path = dirs.config_dir().to_path_buf();
    path.push("spotifyd");
    path.push(CONFIG_FILE_NAME);

    if path.exists() {
        Some(path)
    } else {
        let path: PathBuf = etc_conf.into();

        if path.exists() {
            Some(path)
        } else {
            None
        }
    }
}
#[cfg(all(unix))]
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
    hex::encode(Sha1::digest(name.as_bytes()))
}

pub(crate) struct SpotifydConfig {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    #[allow(unused)]
    pub(crate) use_keyring: bool,
    pub(crate) use_mpris: bool,
    pub(crate) dbus_type: DBusType,
    pub(crate) cache: Option<Cache>,
    pub(crate) backend: Option<String>,
    pub(crate) audio_device: Option<String>,
    pub(crate) audio_format: LSAudioFormat,
    #[allow(unused)]
    pub(crate) control_device: Option<String>,
    #[allow(unused)]
    pub(crate) mixer: Option<String>,
    #[allow(unused)]
    pub(crate) volume_controller: VolumeController,
    pub(crate) initial_volume: Option<u16>,
    pub(crate) device_name: String,
    pub(crate) player_config: PlayerConfig,
    pub(crate) session_config: SessionConfig,
    pub(crate) onevent: Option<String>,
    #[allow(unused)]
    pub(crate) pid: Option<String>,
    pub(crate) shell: String,
    pub(crate) zeroconf_port: Option<u16>,
    pub(crate) device_type: String,
    pub(crate) autoplay: bool,
}

pub(crate) fn get_internal_config(config: CliConfig) -> SpotifydConfig {
    let audio_cache = !config.shared_config.no_audio_cache;

    let size_limit = config.shared_config.max_cache_size;
    let cache = config
        .shared_config
        .cache_path
        .map(|path| {
            Cache::new(
                Some(&path),
                Some(&path),
                audio_cache.then_some(&path),
                size_limit,
            )
        })
        .transpose()
        .unwrap_or_else(|e| {
            warn!("Cache couldn't be initialized: {e}");
            None
        });

    let bitrate: LSBitrate = config
        .shared_config
        .bitrate
        .unwrap_or(Bitrate::Bitrate160)
        .into();

    let audio_format: LSAudioFormat = config
        .shared_config
        .audio_format
        .unwrap_or(AudioFormat::S16)
        .into();

    let backend = config
        .shared_config
        .backend
        .unwrap_or_else(default_backend)
        .to_string();

    let volume_controller = config
        .shared_config
        .volume_controller
        .unwrap_or(VolumeController::SoftVolume);

    let initial_volume: Option<u16> = config
        .shared_config
        .initial_volume
        .and_then(|input| match input.parse::<i16>() {
            Ok(v) if (0..=100).contains(&v) => Some(v),
            _ => {
                warn!("Could not parse initial_volume (must be in the range 0-100)");
                None
            }
        })
        .map(|volume| (volume as i32 * 0xFFFF / 100) as u16);

    let device_name = config
        .shared_config
        .device_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{}@{}", "Spotifyd", gethostname().to_string_lossy()));

    let device_id = device_id(&device_name);

    let normalisation_pregain = config.shared_config.normalisation_pregain.unwrap_or(0.0);

    let dbus_type = config.shared_config.dbus_type.unwrap_or(DBusType::Session);
    let autoplay = config.shared_config.autoplay;

    let device_type = config
        .shared_config
        .device_type
        .unwrap_or(DeviceType::Speaker)
        .to_string();

    let pid = config.pid.map(|f| {
        f.into_os_string()
            .into_string()
            .expect("Failed to convert PID file path to valid Unicode")
    });

    let shell = utils::get_shell().unwrap_or_else(|| {
        info!("Unable to identify shell. Defaulting to \"sh\".");
        "sh".to_string()
    });

    let mut username = config.shared_config.username;
    if username.is_none() {
        info!("No username specified. Checking username_cmd");
        match config.shared_config.username_cmd {
            Some(ref cmd) => match run_program(&shell, cmd) {
                Ok(s) => username = Some(s.trim().to_string()),
                Err(e) => error!("{}", CrateError::subprocess_with_err(&shell, cmd, e)),
            },
            None => info!("No username_cmd specified"),
        }
    }

    let mut password = config.shared_config.password;
    if password.is_none() {
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

    // TODO: when we were on librespot 0.1.5, all PlayerConfig values were available in the
    //  Spotifyd config. The upgrade to librespot 0.2.0 introduces new config variables, and we
    //  should consider adding them to Spotifyd's config system.
    let pc = PlayerConfig {
        bitrate,
        normalisation: config.shared_config.volume_normalisation,
        normalisation_pregain_db: normalisation_pregain,
        gapless: true,
        ..Default::default()
    };

    SpotifydConfig {
        username,
        password,
        use_keyring: config.shared_config.use_keyring,
        use_mpris: config.shared_config.use_mpris.unwrap_or(true),
        dbus_type,
        cache,
        backend: Some(backend),
        audio_device: config.shared_config.device,
        audio_format,
        control_device: config.shared_config.control,
        mixer: config.shared_config.mixer,
        volume_controller,
        initial_volume,
        device_name,
        player_config: pc,
        session_config: SessionConfig {
            user_agent: version::VERSION_STRING.to_string(),
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
        let mut spotifyd_section = SharedConfigValues {
            password: Some("123456".to_string()),
            ..Default::default()
        };

        let global_section = SharedConfigValues {
            username: Some("testUserName".to_string()),
            ..Default::default()
        };

        // The test only makes sense if both sections differ.
        assert_ne!(spotifyd_section, global_section);

        let file_config = FileConfig {
            global: Some(global_section),
            spotifyd: Some(spotifyd_section.clone()),
        };
        let merged_config = file_config.get_merged_sections().unwrap();

        // Add the new field to spotifyd section.
        spotifyd_section.username = Some("testUserName".to_string());
        assert_eq!(merged_config, spotifyd_section);
    }
    #[test]
    fn test_default_backend() {
        let spotifyd_config = get_internal_config(CliConfig::default());
        assert_eq!(
            spotifyd_config.backend.unwrap(),
            default_backend().to_string()
        );
    }
}

[![Dependabot Status](https://api.dependabot.com/badges/status?host=github&repo=Spotifyd/spotifyd)](https://dependabot.com)
[![Github Actions - CD](https://github.com/Spotifyd/spotifyd/workflows/Continuous%20Deployment/badge.svg)](https://github.com/Spotifyd/spotifyd/actions)
[![Github Actions - CI](https://github.com/Spotifyd/spotifyd/workflows/Continuous%20Integration/badge.svg)](https://github.com/Spotifyd/spotifyd/actions)

# Spotifyd <!-- omit in toc -->

> An open source Spotify client running as a UNIX daemon.

Spotifyd streams music
just like the official client, but is more lightweight and supports more
platforms. Spotifyd also supports the Spotify Connect protocol which makes it
show up as a device that can be controlled from the official clients.

**Note:** Spotifyd requires a Spotify Premium account.

- [Installation](#installation)
  - [Provided binaries](#provided-binaries)
  - [Compiling from source](#compiling-from-source)
    - [Feature Flags](#feature-flags)
      - [DBus MPRIS](#dbus-mpris)
    - [Audio Backend](#audio-backend)
      - [PulseAudio](#pulseaudio)
      - [PortAudio](#portaudio)
- [Configuration](#configuration)
  - [CLI options](#cli-options)
  - [Configuration file](#configuration-file)

## Installation

### Provided binaries

We provide pre-built binaries through Github Actions for the more popular platforms: Linux, macOS and ARMv6. You can find them [here](https://github.com/Spotifyd/spotify/releases). For extra integrity, the files' SHA-512 gets calculated and uploaded as well.

The provided binaries come in two flavours, `slim` and `full`. Each are compiled with different features. `slim` only contains the platform's most used audio backend, `full` has also all optional features enabled (see #[Feature Flags]).

### Compiling from source

You can also compile `Spotifyd` yourself, allowing you to make use of feature flags. `Spotifyd` is written in Rust. You can download the toolchain (compiler and package manager) over at [rustup.rs](https://rustup.rs). Follow their instructions to get started.

`Spotifyd` might require additional libraries during build and runtime, depending on your platform. The following table shows the libraries needed for each OS respectively.

| Target Platform | Libraries                                            |
|-----------------|------------------------------------------------------|
| Linux           | libasound2-dev libssl-dev libpulse-dev libdbus-1-dev |
| macOS           | dbus, pkg-config, portaudio                          |

> __Note:__ The package names for Linux are the ones used on Debian based OS (like Ubuntu). You will need to adapt the packages for your distribution.

To install the resulting binary, run 

```bash
cargo install --path .
```

#### Feature Flags

`Spotifyd` is split into a base package plus additional features that can be toggled on or off during compilation. Those can be split into two groups: The audio backends features and additional functionality features.

To enable an additional audio backend, pass `<audio_backend_name>_backend` as a feature flag. We currently support `alsa`, `pulseaudio` and `portaudio`.

`Spotifyd` provides the following additional functionality:

| Feature Flag | Description                                                                         |
|--------------|-------------------------------------------------------------------------------------|
| dbus_keyring | Provides password authentication over the system's keyring (supports all platforms) |
| dbus_mpris   | Provides multimedia key support for Linux only                                      |

> __Note:__ Compiling Spotifyd with all features and the pulseaudio backend on Ubuntu would result in the following command: `cargo build --release --no-default-features --features pulseaudio_backend,dbus_keyring,dbus_mpris`

##### DBus MPRIS

Spotifyd implements 
[DBus MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/), meaning that it can be controlled by some generic media playback controllers such as
[playerctl](https://github.com/acrisci/playerctl/tree/4cf5ba8ad00f47c8db8af0fd20286b050921a6e1)
as well as some tools specifically designed for use with the official Spotify
client such as [sp](https://gist.github.com/wandernauta/6800547).

> __Note:__ Make sure to rename the service name within the `sp` script to `spotifyd`!

The D-Bus server is currently experimental. Enable the `dbus_mpris` feature when
compiling to try it out.

#### Audio Backend

By default, the audio backend is ALSA, as ALSA is available by default on a lot
of machines and requires no extra dependencies. There is also support for
`pulseaudio` and `portaudio`.

##### PulseAudio

To use PulseAudio, compile with the `--features` flag to enable
it:

```bash
cargo build --release --features="pulseaudio_backend"
```

You will need the development package for PulseAudio, as well
as `build-essential` or the equivalent in your distribution.

##### PortAudio

To use PortAudio (works on OSX), compile with the `--features` flag to enable it:

```bash
cargo build --release --no-default-features --features="portaudio_backend"
```

## Configuration

`Spotifyd` is able to run without configuration at all and will assume default values for most of the fields. However, running without configuration will only allow you to connect to it if you're on the same network as the daemon.

### CLI options

`Spotifyd` can be configured using CLI arguments. For a detailed description as well as possible values for each flag, run

```bash
spotifyd --help
```

### Configuration file

`Spotifyd` is able to load configuration values from a file too. The file has to be named `spotifyd.conf` and reside in the user's configuration directory (`~/.config/spotifyd`) or the system configuration directory (`/etc` or `/etc/xdg/spotifyd`).

The configuration file consists of two sections, `global` and `spotifyd`, whereas `spotifyd` takes priority over `global`.

The configuration file has the following format:

```ini
[global]
username = USER                                          # Spotify username
password = PASS                                          # Spotify password
password_cmd = command_that_writes_password_to_stdout    # can be used as alternative to `password`
use_keyring = true                                       # can be used as alternative to `password`
backend = alsa                                           # run `spotifyd --backends` for possible values
device = alsa_audio_device                               # run `aplay -L` for possible values
control = alsa_audio_device                              # device for the mixer, if not the same as 'device'
mixer = PCM
volume_controller = alsa                                 # or alsa_linear, or softvol
on_song_change_hook = command_to_run_on_playback_events
device_name = device_name_in_spotify_connect             # must not contain spaces
bitrate = 160                                            # or 96, or 320
cache_path = cache_directory
no_audio_cache = true                                    # use credentials-only caching
volume_normalisation = true
normalisation_pregain = -10
zeroconf_port = port_number                              # the port used to start the Spotify discovery service 
```

#### Cache path <!-- omit in toc -->

The line `cache_path = /cache_directory` defines the cache path, where Spotify's cache-files are stored. These cache-files are used to avoid re-downloading data when a track is replayed.  Here the cache path is set to store the cache-files in `/cache_directory`. To save space on the system disc you can use another directory, for example `/mount/disk/spotifyCache`. Spotifyd does not create a missing cache path, so the path must exist. The cache path is not expanded by the shell: paths containing e.g. `~/` or `$HOME/` will not work.

#### Alternatives to storing your password in the config file <!-- omit in toc -->

- **`password_cmd`** config entry

  This feature allows you to, in the config file, provide a command that prints your password to `stdout`, which saves you from having to store your
  password in the config file directly. To use it, set the `password_cmd` config entry to the command you would like to use and remove the `password` config entry, which, if present, would take priority.

  For example (using the password-management utility [pass](https://www.passwordstore.org/)).

  ```ini
  # ~/.config/spotifyd/spotifyd.conf
  password_cmd = pass spotify
  ```

- **`use_keyring`** config entry / **`--use-keyring`** CLI flag <!-- omit in toc -->

  This features leverages Linux's DBus Secret Service API 
  ([info](https://www.freedesktop.org/wiki/Specifications/secret-storage-spec/))
  in order to forgo the need to store your password directly in the config file. 
  To use it, compile with the `dbus_keyring` feature and set the `use-keyring` 
  config entry to `true` or pass the `--use-keyring` CLI flag during start to 
  the daemon. Remove the `password` and/or `password_cmd` config entries, which,
  if present, would take priority.

  Your keyring entry needs to have the following attributes set:

  ```
  application: rust-keyring
  service: spotifyd
  username: <your-spotify-username>
  ```

  To add such an entry into your keyring, you can use `secret-tool`, a CLI used 
  to communicate with agents that support the Secret Service API:

  ```bash
  secret-tool store --label='name you choose' application rust-keyring service spotifyd username <your-username>
  ```

#### Shell used to run commands indicated by `password_cmd` or `on_song_changed_hook` <!-- omit in toc -->

If either of these options is given, the shell `spotifyd` will use to run their commands is the shell indicated by the `SHELL` environment variable, if set. If the `SHELL` environment variable is not set, `spotifyd` will use the 
user's default shell, which, on linux and the BSDs, is the shell listed in `/etc/passwd`, and, on macOS, is the shell listed in the output of `dscl . -read /Users/<username> UserShell`.

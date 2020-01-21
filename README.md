# Spotifyd <!-- omit in toc -->

[![Dependabot Status][dependabot-badge]](https://dependabot.com)
[![Github Actions - CD][cd-badge]][github-actions]
[![Github Actions - CI][ci-badge]][github-actions]

> An open source Spotify client running as a UNIX daemon.

Spotifyd streams music just like the official client, but is more lightweight and supports more platforms. Spotifyd also supports the Spotify Connect protocol, which makes it show up as a device that can be controlled from the official clients.

> __Note:__ Spotifyd requires a Spotify Premium account.

- [Installation](#installation)
  - [Provided binaries](#provided-binaries)
  - [Compiling from source](#compiling-from-source)
    - [Feature Flags](#feature-flags)
      - [Media controls](#media-controls)
      - [Audio Backends](#audio-backends)
        - [PulseAudio](#pulseaudio)
        - [PortAudio](#portaudio)
        - [Rodio](#rodio)
- [Configuration](#configuration)
  - [CLI options](#cli-options)
  - [Configuration file](#configuration-file)
- [Running as a systemd service](#running-as-a-systemd-service)
- [Common issues](#common-issues)
- [Contributing](#contributing)
- [Credits](#credits)

## Installation

### Provided binaries

We provide pre-built binaries through GitHub Actions for the more popular platforms: Linux, macOS and ARMv7. You can find them [here](https://github.com/Spotifyd/spotifyd/releases). For extra integrity, the file's SHA-512 gets calculated and uploaded as well.

The provided binaries come in two flavours, `slim` and `full`. Each are compiled with different features. `slim` only contains the platform's most used audio backend, `full` has also all optional features enabled (see [Feature Flags](#feature-flags)).

### Compiling from source

You can also compile `Spotifyd` yourself, allowing you to make use of feature flags. `Spotifyd` is written in Rust. You can download the toolchain (compiler and package manager) over at [rustup.rs](https://rustup.rs). Follow their instructions to get started.

`Spotifyd` might require additional libraries during build and runtime, depending on your platform and the way to compile it (static or dynamic). The following table shows the libraries needed for each OS respectively.

| Target Platform | Libraries                                            |
|-----------------|------------------------------------------------------|
| Fedora          | alsa-lib-devel, make, gcc                            |
| openSUSE        | alsa-devel, make, gcc                                |
| Debian          | libasound2-dev libssl-dev libpulse-dev libdbus-1-dev |
| macOS           | dbus, pkg-config, portaudio                          |

> __Note:__ The package names for Linux are the ones used on Debian based distributions (like Ubuntu). You will need to adapt the packages for your distribution respectively.

To compile the binary, run

```bash
cargo build --release
```

To install the resulting binary, run

```bash
cargo install --path .
```

#### Feature Flags

`Spotifyd` is split into a base package plus additional features that can be toggled on or off during compilation. Those can be split into two groups: The audio backend features that are responsible for playing back the music and additional functionality features, which enhance your experience using `spotifyd`.

To enable an additional audio backend, pass `<audio_backend_name>_backend` as a feature flag. We currently support `alsa`, `pulseaudio` and `portaudio`.

`Spotifyd` provides the following additional functionality:

| Feature Flag | Description                                                                         |
|--------------|-------------------------------------------------------------------------------------|
| dbus_keyring | Provides password authentication over the system's keyring (supports all platforms) |
| dbus_mpris   | Provides multimedia key support (Linux only)                                      |

> __Note:__ Compiling Spotifyd with all features and the pulseaudio backend on Ubuntu would result in the following command: `cargo build --release --no-default-features --features pulseaudio_backend,dbus_keyring,dbus_mpris`

##### Media controls

Spotifyd implements the [MPRIS D-Bus Interface Specification][mpris-specification], meaning that it can be controlled by generic media playback controllers such as [playerctl][playerctl-homepage] as well as some tools specifically designed for use with the official Spotify client such as [sp][sp-homepage].

> __Note:__ Make sure to rename the service name within the `sp` script to `spotifyd`!

Although the code greatly improved, this feature is still considered experimental. Make sure to open an issue if you encounter any issues while using other players to control `spotifyd`.

##### Audio Backends

By default, the audio backend is ALSA, as ALSA is available by default on a lot of machines and usually doesn't require extra dependencies. There is also support for `pulseaudio` and `portaudio`.

> __Note:__ To disable this audio backend, pass `--no-default-features` down during compilation.

###### PulseAudio

To use PulseAudio, compile with the `--features` flag to enable
it:

```bash
cargo build --release --features "pulseaudio_backend"
```

You will need the development package for PulseAudio, as well
as `build-essential` or the equivalent package of your distribution.

###### PortAudio

To use PortAudio (works on macOS), compile with the `--features` flag to enable it:

```bash
cargo build --release --no-default-features --features="portaudio_backend"
```

> __Note:__ It is important that you also pass down `--no-default-features` as macOS doesn't support the `alsa_backend` feature!

###### Rodio

To use Rodio (works on Windows, OSX, Linux), compile with the `--features` flag to enable it:

```bash
cargo build --release --no-default-features --features="rodio_backend"
```

On Linux you will need the development package for alsa and make/gcc. (`libasound2-dev`,`build-essential` on debian, `alsa-lib-devel`,`make`,`gcc` on fedora)

## Configuration

`Spotifyd` is able to run without configuration at all and will assume default values for most of the fields. However, running without configuration will only allow you to connect to it if you're on the same network as the daemon.

> __Note:__ This is currently not possible anymore and under investigation. For more information and updates, take a look at #366.

### CLI options

`Spotifyd` can be configured using CLI arguments. For a detailed description as well as possible values for each flag, run

```bash
spotifyd --help
```

### Configuration file

`Spotifyd` is able to load configuration values from a file too. The file has to be named `spotifyd.conf` and reside in the user's configuration directory (`~/.config/spotifyd`) or the system configuration directory (`/etc` or `/etc/xdg/spotifyd`). This also applies to macOS!

The configuration file consists of two sections, `global` and `spotifyd`, whereas `spotifyd` takes priority over `global`.

The configuration file has the following format:

```ini
[global]
# Your Spotify account name.
username = username

# Your Spotify account password.
password = password

# A command that gets executed and can be used to
# retrieve your password.
# The command should return the password on stdout.
#
# This is an alternative to the `password` field. Both
# can't be used simultaneously.
password_cmd = command_that_writes_password_to_stdout

# If set to true, `spotifyd` tries to look up your
# password in the system's password storage.
#
# This is an alternative to the `password` field. Both
# can't be used simultaneously.
use_keyring = true

# The audio backend used to play the your music. To get
# a list of possible backends, run `spotifyd --help`.
backend = alsa

# The alsa audio device to stream audio to. To get a
# list of valid devices, run `aplay -L`,
device = alsa_audio_device

# The alsa control device. By default this is the same
# name as the `device` field.
control = alsa_audio_device

# The alsa mixer used by `spotifyd`.
mixer = PCM

# The volume controller. Each one behaves different to
# volume increases. For possible values, run
# `spotifyd --help`.
volume_controller = alsa

# A command that gets executed in your shell after each song changes.
on_song_change_hook = command_to_run_on_playback_events

# The name that gets displayed under the connect tab on
# official clients. Spaces are not allowed!
device_name = device_name_in_spotify_connect

# The audio bitrate. 96, 160 or 320 kbit/s
bitrate = 160

# The director used to cache audio data. This setting can save
# a lot of bandwidth when activated, as it will avoid re-downloading
# audio files when replaying them.
#
# Note: The file path does not get expanded. Environment variables and
# shell placeholders like $HOME or ~ don't work!
cache_path = cache_directory

# If set to true, audio data does NOT get cached.
no_audio_cache = true

# If set to true, enables volume normalisation between songs.
volume_normalisation = true

# The normalisation pregain that is applied for each song.
normalisation_pregain = -10

# The port `spotifyd` uses to announce its service over the network.
zeroconf_port = 1234

# The proxy `spotifyd` will use to connect to spotify.
proxy = http://proxy.example.org:8080
```

#### Alternatives to storing your password in the config file <!-- omit in toc -->

- **`password_cmd`** config entry

  This feature allows you to provide a command that prints your password to `stdout`, which saves you from having to store your password in the config file directly. To use it, set the `password_cmd` config entry to the command you would like to use and remove the `password` config entry.

  For example (using the password-management utility [pass][pass-homepage]).

  ```ini
  # ~/.config/spotifyd/spotifyd.conf
  password_cmd = pass spotify
  ```

- **`use_keyring`** config entry / **`--use-keyring`** CLI flag <!-- omit in toc -->

  This features leverages [Linux's DBus Secret Service API][secret-storage-specification] or native macOS keychain in order to forgo the need to store your password directly in the config file. To use it, complile with the `dbus_keyring` feature and set the `use-keyring` config entry to `true` or pass the `--use-keyring` CLI flag  during start to the daemon. Remove the `password` and/or `password_cmd` config entries.

  Your keyring entry needs to have the following attributes set:

  ```
  application: rust-keyring
  service: spotifyd
  username: <your-spotify-username>
  ```

  To add such an entry into your keyring, you can use `secret-tool`, a CLI used to communicate with agents that support the Secret Service API:

  ```bash
  secret-tool store --label='name you choose' application rust-keyring service spotifyd username <your-username>
  ```

  You can use the keychain GUI on macOS to add an item respectively.

#### Shell used to run commands indicated by `password_cmd` or `on_song_changed_hook` <!-- omit in toc -->

If either of these options is given, the shell `spotifyd` will use to run its commands is the shell indicated by the `SHELL` environment variable, if set. If the `SHELL` environment variable is not set, `spotifyd` will use the user's default shell, which, on Linux and BSD, is the shell listed in `/etc/passwd`. On macOS it is the shell listed in the output of `dscl . -read /Users/<username> UserShell`.

## Running as a systemd service

A `systemd.service` unit file is provided to help run spotifyd as a service on systemd-based systems. The file `contrib/spotifyd.service` should be copied to either:

```
/etc/systemd/user/
~/.config/systemd/user/
```

Packagers of systemd-based distributions are encouraged to include the file in the former location. End-user should prefer the latter. It should be noted that some targets are not available when running under the user directory, such as `network-online.target`.

Control of the daemon is handed over to systemd. The following example commands will run the service once and enable the service to always run on login in the future respectively:

```
systemctl --user start spotifyd.service
systemctl --user enable spotifyd.service
```

## Common issues

- Spotifyd will not work without Spotify Premium
- The device name cannot contain spaces
- Launching in discovery mode (username and password left empty) makes the daemon undiscoverable from within the app (tracking issue #373)

## Contributing

We always appreciate help during the development of `spotifyd`! If you are new to programming, open source or Rust in general, take a look at issues tagged with [`good first issue`][good-first-issues]. These normally are easy to resolve and don't take much time to implement.

## Credits

This project would not have been possible without the amazing reverse engineering work done in [librespot](https://github.com/librespot-org/librespot), mostly by [plietar](https://github.com/plietar).

<!-- This section contains all links used within the document. This prevents cluttering and makes reading the raw markdown a lot easier -->
[github-actions]: https://github.com/Spotifyd/spotifyd/actions
[good-first-issues]: https://github.com/Spotifyd/spotifyd/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22
[mpris-specification]: https://specifications.freedesktop.org/mpris-spec/latest/
[pass-homepage]: https://www.passwordstore.org/
[playerctl-homepage]: https://github.com/acrisci/playerctl/
[secret-storage-specification]: https://www.freedesktop.org/wiki/Specifications/secret-storage-spec/
[sp-homepage]: https://gist.github.com/wandernauta/6800547

[cd-badge]: https://github.com/Spotifyd/spotifyd/workflows/Continuous%20Deployment/badge.svg
[ci-badge]: https://github.com/Spotifyd/spotifyd/workflows/Continuous%20Integration/badge.svg
[dependabot-badge]: https://api.dependabot.com/badges/status?host=github&repo=Spotifyd/spotifyd

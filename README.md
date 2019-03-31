# Spotifyd
An open source Spotify client running as a UNIX daemon. Spotifyd streams music
just like the official client, but is more lightweight and supports more
platforms. Spotifyd also supports the Spotify Connect protocol which makes it
show up as a device that can be controlled from the official clients.

Spotifyd requires a Spotify Premium account.

### What happened to the old spotifyd, written in C?
Unfortunately, Spotify decided to kill the libspotify library we used, and
hence we had no choice but to rewrite everything.

# Installing
Travis CI builds binaries for systems running Linux on AMD64 and ARMv6 which
should run on any Raspberry Pi model. The binaries can be found
[here](https://github.com/Spotifyd/spotifyd/releases/latest). Other systems
have to build from source for now. You will need the ALSA package for your
distribution, e.g. libasound2-dev on Ubuntu.

Detailed install instructions can be found on the [wiki](https://github.com/Spotifyd/spotifyd/wiki).

## Build from source
The [Rust compiler and Cargo package](https://www.rust-lang.org/learn/get-started)
 manager are needed:
```
cargo build --release
```
The resulting binary will be placed in `target/release/spotifyd`.

Alternatively, the package can be installed into the user's home directory
by running the following command:
```
cargo install --path .
```
This method also allows further configuration by specifing more feature flags
as shown [further down](#command-line-arguments).

The default is to build spotifyd with an ALSA backend, but it is possible
to build with other audio backends, making Spotifyd availible on platforms
other than Linux, by adding the `--no-default-features` argument to cargo
and supplying an alternative backend (see the _Configuration_ section).

# Configuration
Spotifyd will search for a file name `spotifyd.conf` in the XDG config
directories (meaning, a users local config is placed in
`~/.config/spotifyd/spotifyd.conf`, a system wide config is in
`/etc/spotifyd.conf` or in `/etc/xdg/spotifyd/spotifyd.conf`) and has the following format:
```
[global]
username = USER
password = PASS
#use-keyring = true
backend = alsa
device = alsa_audio_device # Given by `aplay -L`
mixer = PCM
volume-control = alsa # or alsa_linear, or softvol
#onevent = command_run_on_playback_event
device_name = name_in_spotify_connect # Cannot contain spaces
bitrate = 96|160|320
cache_path = cache_directory
volume-normalisation = true
normalisation-pregain = -10
```
Every field is optional; `Spotifyd` can even run without a configuration file.
Options can also be placed in a `[spotifyd]` section which takes priority over
the `[global]` section. This is useful when you run applications related to
`Spotifyd` which shares some, but not all, options with `Spotifyd`.

Values can be surrounded by double quotes (") which is useful if the value contains
the comment character (#).

Instead of writing down your password into the config file, `Spotifyd` supports the
Linux Secret Service API when compiled with the `dbus_keyring` feature. To enable 
this feature, you have to set the `use-keyring` config entry to `true` or pass the
`--use-keyring` CLI flag during start to the daemon.

The keyring entry needs to have the following attributes set:
```
application: rust-keyring
service: spotifyd
username: <your-spotify-username>
```

To add such an entry, you can use `secret-tool`, a CLI used to communicate with agents
that support the Linux Secret Service API:

```
$ secret-tool --label='entry name that you can choose' application rust-keyring service spotifyd username <your-username>
```

## Command Line Arguments
`spotifyd --help` gives an up-to-date list of available arguments. The command
line arguments allows for specifying a PID file, setting a verbose mode, run in
no-daemon mode, among other things.

## Audio Backend
By default, the audio backend is ALSA, as ALSA is available by default on a lot
of machines and requires no extra dependencies. There is also support for
`pulseaudio` and `portaudio`. 

### PulseAudio
To use PulseAudio, compile with the `--features` flag to enable
it:
```
cargo build --release --features pulseaudio_backend
```
You will need the development package for PulseAudio, as well
as `build-essential` or the equivalent in your distribution.

### PortAudio
To use PortAudio (works on OSX), compile with the `--features` flag to enable it:
```
cargo build --release --no-default-features --features portaudio_backend
```
You will need the development package for PortAudio (`brew install portaudio`), as well
as `build-essential` or the equivalent in your distribution.


# Usage
Spotifyd communicates over the Spotify Connect protocol, meaning that it can be
controlled from the official Spotify client on Android/iOS/Desktop.

For a more lightweight and scriptable alternative, there is
the [Spotify Connect
API](https://developer.spotify.com/web-api/web-api-connect-endpoint-reference/).

## D-Bus MPRIS
Spotifyd implements [D-Bus
MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/) which means
it can be controlled by some generic media playback controllers such as
[playerctl](https://github.com/acrisci/playerctl/tree/4cf5ba8ad00f47c8db8af0fd20286b050921a6e1)
as well as some tools specifically designed for use with the official Spotify
client such as [sp](https://gist.github.com/wandernauta/6800547) (requires
changing the DBus service name to spotifyd instead of spotify).

The D-Bus server is currently experimental. Enable the `dbus_mpris` feature when
compiling to try it out.

## Running as a systemd service

A systemd.service unit file is provided to help run spotifyd as a service on
systemd-based systems. The file `contrib/spotifyd.service` should be copied to
either:

    /etc/systemd/user/
    ~/.config/systemd/user/

Packagers of systemd-based distributions are encouraged to include the file in
the former location. End-user should prefer the latter.

It should be noted that some targets are not available when running under the
user directory, such as `network-online.target`.

Control of the daemon is then done via systemd. The following example commands
will run the service once and enable the service to always run on login in the
future respectively:

    systemctl --user start spotifyd.service
    systemctl --user enable spotifyd.service

# Logging
In `--no-daemon` mode, the log is written to standard output, otherwise it is
written to syslog, and where it's written can be configured in your system
logger.

The verbose mode adds more information; please enable this mode when submitting
a bug report.

# Common Issues

* Spotifyd will not work without Spotify Premium
* The device name cannot contain spaces

# Contributing

New PR's are always welcome! Lately did introduce two new tools to maintain some level of code consistency, `clippy` and `rustfmt`. To install them, run the following command: 

```rustup add component clippy rustfmt```

Make sure to check that `clippy` exits without any errors:

```cargo clippy --all-targets --all-features -- -D warnings```.

Please also format your source code before submitting the PR by running the following command: 

```rustfmt src/**/*.rs```

# Credits
This project would not have been possible without the amazing reverse
engineering work done in [librespot](https://github.com/plietar/librespot),
mostly by [plietar](https://github.com/plietar).

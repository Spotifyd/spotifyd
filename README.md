# Spotifyd
An open source Spotify client running as a UNIX daemon. Spotifyd is more
lightweight than the official client, and is available on more platforms.

### What happened to the old spotifyd, written in C?
Unfortunately, Spotify decided to kill the libspotify library we used, and
hence we had no choice but to rewrite everything.

# Installing
Travis CI builds binary for systems running Linux on AMD64 and ARMv7, which
should run on Raspberry Pi model 2 and 3. The binaries can be found
[here](https://github.com/Spotifyd/spotifyd/releases/latest). Other systems
have to build from source for now.

## Build from source
The [Rust compiler and Cargo package](https://www.rust-lang.org/en-US/)
 manager are needed:
```
cargo build --release
```
The resulting binary will be placed in `target/release/spotifyd`.

The default is to build spotifyd with an alsa backend, but it is possible
to build with other audio backends, making Spotifyd availible on platforms
other than Linux, by adding the `--no-default-features` argument to cargo
and supplying an alternative backend (see the _Configuration_ section).

# Configuration
Spotifyd will search for a file name `spotifyd.conf` in the XDG config
directories (meaning, a users local config is placed in
`~/.config/spotifyd/spotifyd.conf`), and has the following format:
```
[global]
username = USER
password = PASS
backend = alsa
device = alsa_audio_device # Given by `aplay -L`
onstart = command_run_on_playback_start
onstop = command_run_on_playback_stop
device_name = name_in_spotify_connect
bitrate = 96|160|320
cache = cache_directory
```
Every field is optional, `Spotifyd` can even run without a configuration file.
Options can also be placed in a `[spotifyd]` section, which takes priority over
the `[global]` section, which is useful when you run applications related to
`Spotifyd`, which shares some but not all options with `Spotifyd`.

Values can be surrounded by double quotes ("), which is useful if it contains
the comment character (#).

## Command Line Arguments
`spotifyd --help` gives an up to date list of available arguments. The command
line arguments allows for specifying a PID file, setting a verbose mode, run in
no-daemon mode, among othre things.

## Audio Backend
By default, the audio backend is alsa, as that is available by default on a lot
of machines, and requires no extra dependencies. There is also support for
`pulseaudio` and `portaudio`. They require you to recompile with flags enabling
them:
```
cargo build --release --features "portaudio pulseaudio"
```
You will need the development packages for portaudio and/or pulseaudio, as well
as "build-essentials" or the equivalent in your distribution.

# Usage
Spotifyd communicates over the Spotify Connect protocol, meaning that it can be
controlled from the official Spotify client on Android/iOS/Desktop.

For a more lightweight, and scriptable alternative, there is
[spotifyd-http](https://github.com/Spotifyd/spotifyd-http), which is a work in
progress but already supports basic tasks.

# Logging
In `--no-daemon` mode, the log is written to standard output, otherwise it is
written to syslog, and where it's written can be configured in your system
logger.

The verbose mode adds more information, please enable this mode when submitting
a bug report.

# Credits
This project would not have been possible without the amazing reverse
engineering work done in [librespot](https://github.com/plietar/librespot),
mostly by [plietar](https://github.com/plietar).

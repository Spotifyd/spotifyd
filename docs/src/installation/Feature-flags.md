# Feature Flags

`Spotifyd` is split into a base package plus additional features that can be toggled on or off during compilation. Those can be split into two groups: The audio backend features that are responsible for playing back the music and additional functionality features, which enhance your experience using `spotifyd`.

To enable an additional audio backend, pass `<audio_backend_name>_backend` as a feature flag. We currently support `alsa`, `pulseaudio` and `portaudio`.

`Spotifyd` provides the following additional functionality:

| Feature Flag | Description                                                                         |
|--------------|-------------------------------------------------------------------------------------|
| dbus_keyring | Provides password authentication over the system's keyring (supports all platforms) |
| dbus_mpris   | Provides multimedia key support (Linux only)                                      |

> __Note:__ Compiling Spotifyd with all features and the pulseaudio backend on Ubuntu would result in the following command: `cargo build --release --no-default-features --features pulseaudio_backend,dbus_keyring,dbus_mpris`

## Media controls

Spotifyd implements the [MPRIS D-Bus Interface Specification][mpris-specification], meaning that it can be controlled by generic media playback controllers such as [playerctl][playerctl-homepage] as well as some tools specifically designed for use with the official Spotify client such as [sp][sp-homepage].

> __Note:__ Make sure to rename the service name within the `sp` script to `spotifyd`!

Although the code greatly improved, this feature is still considered experimental. Make sure to open an issue if you encounter any issues while using other players to control `spotifyd`.

## Audio Backends

By default, the audio backend is ALSA, as ALSA is available by default on a lot of machines and usually doesn't require extra dependencies. There is also support for `pulseaudio` and `portaudio`.

> __Note:__ To disable this audio backend, pass `--no-default-features` down during compilation.

### PulseAudio

To use PulseAudio, compile with the `--features` flag to enable
it:

```bash
cargo build --release --features "pulseaudio_backend"
```

You will need the development package for PulseAudio, as well
as `build-essential` or the equivalent package of your distribution.

### PortAudio

To use PortAudio (works on macOS), compile with the `--features` flag to enable it:

```bash
cargo build --release --no-default-features --features="portaudio_backend"
```

> __Note:__ It is important that you also pass down `--no-default-features` as macOS doesn't support the `alsa_backend` feature!

### Rodio

To use Rodio (works on Windows, OSX, Linux), compile with the `--features` flag to enable it:

```bash
cargo build --release --no-default-features --features="rodio_backend"
```

On Linux you will need the development package for alsa and make/gcc. (`libasound2-dev`,`build-essential` on debian, `alsa-lib-devel`,`make`,`gcc` on fedora)

[mpris-specification]: https://specifications.freedesktop.org/mpris-spec/latest/

### JACK Audio Connection Kit

To use the [JACK](http://jackaudio.org) backend on Linux, compile with the `--features` flag to enable it:

```bash
cargo build --release --no-default-features --features="rodiojack_backend"
```

You will need the development packages for alsa, make/gcc, and JACK. (`libasound2-dev`, `build-essential`, and `libjack-dev` on Debian; `alsa-lib-devel`, `make`, `gcc`, and `jack-audio-connection-kit-devel` on Fedora.)

> __Note__: when Spotifyd starts with this backend, it will create a JACK output device named `cpal_client_out` with two ports: `out_0` for the left channel and `out_1` for the right.

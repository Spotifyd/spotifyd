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

The [Rust compiler and Cargo package manager](https://www.rust-lang.org/learn/get-started)
are needed:

```bash
cargo build --release
```

The resulting binary will be placed in `target/release/spotifyd`.

Alternatively, the package can be installed into the user's home directory
by running the following command:

```bash
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
directories (meaning, a user's local config is placed in
`~/.config/spotifyd/spotifyd.conf`, a system-wide config in
`/etc/spotifyd.conf` or `/etc/xdg/spotifyd/spotifyd.conf`) and has the following format:

```ini
# ~/.config/spotifyd/spotifyd.conf
[global]
username = USER
password = PASS
# password_cmd = command_that_writes_password_to_stdout  # can be used as alternative to `password`
# use-keyring = true                                     # can be used as alternative to `password`
backend = alsa                                           # run `spotifyd --backends` for possible values
device = alsa_audio_device                               # run `aplay -L` for possible values
# control = alsa_audio_device                            # device for the mixer, if not the same as 'device'
mixer = PCM
volume-control = alsa                                    # or alsa_linear, or softvol
# onevent = command_to_run_on_playback_events
device_name = device_name_in_spotify_connect             # must not contain spaces
bitrate = 160                                            # or 96, or 320
cache_path = cache_directory
volume-normalisation = true
normalisation-pregain = -10
```

Every field is optional; `Spotifyd` can even run without a configuration file.
Options can also be placed in a `[spotifyd]` section which takes priority over
the `[global]` section. This is useful when you run applications related to
`Spotifyd` which shares some, but not all, options with `Spotifyd`.

Values can be surrounded by double quotes (") which is useful if the value 
contains the comment character (#).

**Cache path**

The line `cache_path = /cache_directory` defines the cache path, where Spotify's 
cache-files are stored. These cache-files are used to avoid re-downloading data 
when a track is replayed.  Here the cache path is set to store the cache-files 
in `/cache_directory`. To save space on the system disc you can use another 
directory, for example `/mount/disk/spotifyCache`. Spotifyd does not create a 
missing cache path, so the path must exist. The cache path is not expanded by 
the shell: paths containing e.g. `~/` or `$HOME/` will not work.

**Alternatives to storing your password in the config file**

- **`password_cmd`** config entry

  This feature allows you to, in the config file, provide a command that 
  prints your password to `stdout`, which saves you from having to store your
  password in the config file directly. To use it, set the `password_cmd` config 
  entry to the command you would like to use and remove the `password` config 
  entry, which, if present, would take priority. 

  For example (using the password-management utility 
  [pass](https://www.passwordstore.org/)) ...

  ```ini
  # ~/.config/spotifyd/spotifyd.conf
  password_cmd = pass spotify
  ```


- **`use-keyring`** config entry / **`--use-keyring`** CLI flag

  This features leverages Linux's DBus Secret Service API 
  ([info](https://www.freedesktop.org/wiki/Specifications/secret-storage-spec/))
  in order to forgo the need to store your password directly in the config file. 
  To use it, complile with the `dbus_keyring` feature and set the `use-keyring` 
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

**Shell used to run commands indicated by `password_cmd` or `onevent`**

If either of these options is given, the shell `spotifyd` will use to run 
their commands is the shell indicated by the `SHELL` environment variable, if 
set. If the `SHELL` environment variable is not set, `spotifyd` will use the 
user's default shell, which, on linux and the BSDs, is the shell listed in 
`/etc/passwd`, and, on macOS, is the shell listed in the output of 
`dscl . -read /Users/<username> UserShell`.

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

```bash
cargo build --release --features="pulseaudio_backend"
```

You will need the development package for PulseAudio, as well
as `build-essential` or the equivalent in your distribution.

### PortAudio
To use PortAudio (works on OSX), compile with the `--features` flag to enable it:

```bash
cargo build --release --no-default-features --features="portaudio_backend"
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

Spotifyd implements 
[D-Bus MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/) which 
means it can be controlled by some generic media playback controllers such as
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

# Credits

This project would not have been possible without the amazing reverse
engineering work done in [librespot](https://github.com/plietar/librespot),
mostly by [plietar](https://github.com/plietar).

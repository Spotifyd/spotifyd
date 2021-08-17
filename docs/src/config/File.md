# Configuration file

`Spotifyd` is able to load configuration values from a [TOML](https://toml.io/en/v0.5.0) file too. The file has to be named `spotifyd.conf` and reside in the user's configuration directory (`~/.config/spotifyd`) or the system configuration directory (`/etc` or `/etc/xdg/spotifyd`). This also applies to macOS!

The configuration file consists of two sections, `global` and `spotifyd`, whereas `spotifyd` takes priority over `global`.

The configuration file has the following format:

```toml
[global]
# Your Spotify account name.
username = "username"

# Your Spotify account password.
password = "password"

# A command that gets executed and can be used to
# retrieve your password.
# The command should return the password on stdout.
#
# This is an alternative to the `password` field. Both
# can't be used simultaneously.
password_cmd = "command_that_writes_password_to_stdout"

# If set to true, `spotifyd` tries to look up your
# password in the system's password storage.
#
# This is an alternative to the `password` field. Both
# can't be used simultaneously.
use_keyring = true

# If set to true, `spotifyd` tries to bind to dbus (default is the session bus)
# and expose MPRIS controls. When running headless, without the session bus,
# you should set this to false, to avoid errors. If you still want to use MPRIS,
# have a look at the `dbus_type` option.
use_mpris = true

# The bus to bind to with the MPRIS interface.
# Possible values: "session", "system"
# The system bus can be used if no graphical session is available
# (e.g. on headless systems) but you still want to be able to use MPRIS.
# NOTE: You might need to add appropriate policies to allow spotifyd to
# own the name.
dbus_type = "session"

# The audio backend used to play music. To get
# a list of possible backends, run `spotifyd --help`.
backend = "alsa" # use portaudio for macOS [homebrew]

# The alsa audio device to stream audio. To get a
# list of valid devices, run `aplay -L`,
device = "alsa_audio_device"  # omit for macOS

# The alsa control device. By default this is the same
# name as the `device` field.
control = "alsa_audio_device"  # omit for macOS

# The alsa mixer used by `spotifyd`.
mixer = "PCM"  # omit for macOS

# The volume controller. Each one behaves different to
# volume increases. For possible values, run
# `spotifyd --help`.
volume_controller = "alsa"  # use softvol for macOS

# A command that gets executed in your shell after each song changes.
on_song_change_hook = "command_to_run_on_playback_events"

# The name that gets displayed under the connect tab on
# official clients. Spaces are not allowed!
device_name = "device_name_in_spotify_connect"

# The audio bitrate. 96, 160 or 320 kbit/s
bitrate = 160

# The directory used to cache audio data. This setting can save
# a lot of bandwidth when activated, as it will avoid re-downloading
# audio files when replaying them.
#
# Note: The file path does not get expanded. Environment variables and
# shell placeholders like $HOME or ~ don't work!
cache_path = "cache_directory"

# The maximal size of the cache directory in bytes
# The example value corresponds to ~ 1GB
max_cache_size = 1000000000

# If set to true, audio data does NOT get cached.
no_audio_cache = true

# Volume on startup between 0 and 100
# NOTE: This variable's type will change in v0.4, to a number (instead of string)
initial_volume = "90"

# If set to true, enables volume normalisation between songs.
volume_normalisation = true

# The normalisation pregain that is applied for each song.
normalisation_pregain = -10

# After the music playback has ended, start playing similar songs based on the previous tracks.
autoplay = true

# The port `spotifyd` uses to announce its service over the network.
zeroconf_port = 1234

# The proxy `spotifyd` will use to connect to spotify.
proxy = "http://proxy.example.org:8080"

# The displayed device type in Spotify clients.
# Can be unknown, computer, tablet, smartphone, speaker, t_v,
# a_v_r (Audio/Video Receiver), s_t_b (Set-Top Box), and audio_dongle.
device_type = "speaker"
```

## Alternatives to storing your password in the config file <!-- omit in toc -->

- use zeroconf authentication from Spotify Connect

  Spotifyd is able to advertise itself on the network without credentials. To enable this, you must omit / comment any `username` / `username_cmd` or `password` / `password_cmd` in the configuration. Spotifyd will receive an authentication blob from Spotify when you choose it from the devices list.

  > __Note:__ If you choose to go with this, it is also recommended to omit the `cache_path` and `cache_directory` options. Otherwise the first user to connect to the service will have its authentication blob cached by the service and nobody else will be able to connect to the service without clearing the cache.

  This way, a Spotifyd instance can also be made available to multiple users.

  For more information, have a look at the [librespot documentation][librespot-docs].

- **`password_cmd`** config entry

  This feature allows you to provide a command that prints your password to `stdout`, which saves you from having to store your password in the config file directly. To use it, set the `password_cmd` config entry to the command you would like to use and remove the `password` config entry.

  For example (using the password-management utility [pass][pass-homepage]).

  ```toml
  # ~/.config/spotifyd/spotifyd.conf
  password_cmd = "pass spotify"
  ```

- **`use_keyring`** config entry / **`--use-keyring`** CLI flag <!-- omit in toc -->

  This features leverages [Linux's DBus Secret Service API][secret-storage-specification] or native macOS keychain in order to forgo the need to store your password directly in the config file. To use it, compile with the `dbus_keyring` feature and set the `use-keyring` config entry to `true` or pass the `--use-keyring` CLI flag  during start to the daemon. Remove the `password` and/or `password_cmd` config entries.

  Your keyring entry needs to have the following attributes set:

  ```yaml
  application: rust-keyring
  service: spotifyd
  username: <your-spotify-username>
  ```

  To add such an entry into your keyring, you can use `secret-tool`, a CLI used to communicate with agents that support the Secret Service API:

  ```bash
  secret-tool store --label='name you choose' application rust-keyring service spotifyd username <your-username>
  ```

  You can use the keychain GUI on macOS to add an item respectively, or with the built-in `security` tool:

  ```bash
  security add-generic-password -s spotifyd -D rust-keyring -a <your username> -w
  ```

## Shell used to run commands indicated by `password_cmd` or `on_song_changed_hook` <!-- omit in toc -->

If either of these options is given, the shell `spotifyd` will use to run its commands is the shell indicated by the `SHELL` environment variable, if set. If the `SHELL` environment variable is not set, `spotifyd` will use the user's default shell, which, on Linux and BSD, is the shell listed in `/etc/passwd`. On macOS it is the shell listed in the output of `dscl . -read /Users/<username> UserShell`.

[pass-homepage]: https://www.passwordstore.org/
[playerctl-homepage]: https://github.com/altdesktop/playerctl
[secret-storage-specification]: https://www.freedesktop.org/wiki/Specifications/secret-storage-spec/
[sp-homepage]: https://gist.github.com/wandernauta/6800547
[librespot-docs]: https://github.com/librespot-org/librespot/blob/master/docs/authentication.md#zeroconf-based-authentication

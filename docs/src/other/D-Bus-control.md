# D-Bus control

`Spotifyd` can be configured to bind to D-Bus, exposing controls including standard MPRIS interfaces.

To configure D-Bus, see `use_mpris` and `dbus_type` in the [configuration file](../config/File.md). `Spotifyd` must also be built with the `dbus_mpris` feature, which is available in the `full` flavour of the [provided binaries](https://github.com/Spotifyd/spotifyd/releases).

The D-Bus service name will be in the format `org.mpris.MediaPlayer2.spotifyd.instance<number>`, where `<number>` is the process id.

## Interfaces

### MPRIS

The `org.mpris.MediaPlayer2` and `org.mpris.MediaPlayer2.Player` interfaces from the [MPRIS specification](https://specifications.freedesktop.org/mpris-spec/latest/) are implemented.

Note the `Volume` property of the `org.mpris.MediaPlayer2.Player` interface is read-only, despite supporting writes in the specification.

### Spotifyd Controls

The `rs.spotifyd.Controls` interface includes additional non-standard controls.

- Method `TransferPlayback`: transfers Spotify playback to `spotifyd`
- Method `VolumeUp`: increases player volume
- Method `VolumeDown`: decreases player volume

## Usage

`Spotifyd` can be controlled by applications which support MPRIS such as the [playerctl](https://github.com/altdesktop/playerctl) command-line utility.

The `dbus-send` command can also be used to control `spotifyd`. For example:

- Find the service registered by `spotifyd`: `dbus-send --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.ListNames | grep spotifyd`
- Transfer playback to `spotifyd`: `dbus-send --print-reply --dest=org.mpris.MediaPlayer2.spotifyd.instancexxx /rs/spotifyd/Controls rs.spotifyd.Controls.TransferPlayback`
- Get metadata for the current track: `dbus-send --print-reply --dest=org.mpris.MediaPlayer2.spotifyd.instancexxx /org/mpris/MediaPlayer2 org.freedesktop.DBus.Properties.Get string:org.mpris.MediaPlayer2.Player string:Metadata`

## Troubleshooting

### "Failed to initialize DBus connection" on a headless system

Where no graphical session is available, the system bus can be used by setting the `dbus_type` configuration option to `system`.

### "Failed to register dbus player name" using the system bus

`Spotifyd` may not have permission to register the D-Bus service due to D-Bus security policies. It should be granted permission to own any service with the prefix "org.mpris.MediaPlayer2.spotifyd".

For example, this statement can be added to the default policy in `/usr/share/dbus-1/system.conf`.

```xml
<allow own_prefix="org.mpris.MediaPlayer2.spotifyd"/>
```

It may also be necessary to add a statement to allow clients to send messages.

```xml
<allow send_destination_prefix="org.mpris.MediaPlayer2.spotifyd"/>
```

Make sure to reload the D-Bus configuration after making changes. For example `sudo systemctl reload dbus`.

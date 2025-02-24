# Using D-Bus to control spotifyd

If MPRIS support is built into your version and enabled (`--use-mpris` cli flag / `use_mpris = true` in config), `spotifyd` exposes some interfaces via D-Bus through which it provides information and can be controlled.

Most of the time, you won't have to worry to much about the details, since tools like `playerctl` work out of the box with `spotifyd`. If you have some custom requirements or want to write custom scripts to control `spotifyd`, this section is for you.

## Available Interfaces

Directly after startup, no interfaces will be available. Once we are connected to Spotify, `spotifyd` will request the name `rs.spotifyd.instance$PID` (where `PID=$(pidof spotifyd)`) and expose the interface `rs.spotifyd.Controls`.

As soon as we are the playback device (e.g. because we are selected from another client or the `TransferPlayback` method has been called), `spotifyd` will additionally expose the MPRIS interfaces and request the name `org.mpris.MediaPlayer2.spotifyd.instance$PID`.

### Spotifyd Controls

The `rs.spotifyd.Controls` interface exposes a few useful controls that are available even if we're not the active playback device.

- Method `TransferPlayback`: transfers Spotify playback to `spotifyd`
- Method `VolumeUp`: increases player volume
- Method `VolumeDown`: decreases player volume

Examples:
```bash
dest=rs.spotifyd.instance$(pidof spotifyd)
# increase volume
dbus-send --print-reply --dest=$dest /rs/spotifyd/Controls rs.spotifyd.Controls.VolumeUp
# become the active playback device
dbus-send --print-reply --dest=$dest /rs/spotifyd/Controls rs.spotifyd.Controls.TransferPlayback
```

### MPRIS

The `org.mpris.MediaPlayer2` and `org.mpris.MediaPlayer2.Player` interfaces from the [MPRIS specification](https://specifications.freedesktop.org/mpris-spec/latest/) are implemented.

Example usage:
```bash
dest=org.mpris.MediaPlayer2.spotifyd.instance$(pidof spotifyd)
# Start playback of some Spotify URI
dbus-send --print-reply --dest=$dest /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player.OpenUri string:spotify:track:4PTG3Z6ehGkBFwjybzWkR8
# Get metadata of the currently playing track
dbus-send --print-reply --dest=$dest /org/mpris/MediaPlayer2 org.freedesktop.DBus.Properties.Get string:org.mpris.MediaPlayer2.Player string:Metadata
```

## Examples

Starting Playback without Client:
```bash
#!/bin/bash

# optionally, we can start `spotifyd` here
if ! pidof -q spotifyd
then
  spotifyd --use-mpris
fi

dest=rs.spotifyd.instance$(pidof spotifyd)

wait_for_name() {
  dst=$1
  counter=0

  # check if controls are available
  until [ $counter -gt 10 ] || (dbus-send --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.ListNames | grep -q "$dest")
  do
    sleep 0.3
    ((counter++))
  done

  if [ $counter -gt 10 ]
  then
    echo "waiting for spotifyd timed out" >&1
    exit 1
  fi
}

controls_name=rs.spotifyd.instance$(pidof spotifyd)
wait_for_name $controls_name
echo "Transferring Playback"
dbus-send --print-reply --dest=$controls_name /rs/spotifyd/Controls rs.spotifyd.Controls.TransferPlayback

# if URI is specified, start the playback there
if [ -n "$1" ]
then
  uri="$1"
  mpris_name=org.mpris.MediaPlayer2.spotifyd.instance$(pidof spotifyd)
  wait_for_name $mpris_name
  echo "Starting Playback of $uri"
  dbus-send --print-reply --dest=$mpris_name /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player.OpenUri "string:$uri"
else
  echo "Hint: specify an argument to start playback of a specific Spotify URI"
fi
```

Sleep Timer:
```bash
#!/bin/bash

usage() {
  echo "Usage: $0 <timeout>" >&1
  exit 1
}

[ -n "$1" ] || usage

echo "Sleeping for $1 seconds"
sleep $1

dest=org.mpris.MediaPlayer2.spotifyd.instance$(pidof spotifyd)
dbus-send --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.ListNames | grep -q "$dest"

if [ "$?" = "0" ]
then
  dbus-send --print-reply --dest=$dest /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player.Stop
  # alternatively just pause:
  # dbus-send --print-reply --dest=$dest /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player.Pause
else
  echo "No active spotifyd playback."
fi
```

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

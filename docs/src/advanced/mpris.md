# MPRIS on headless systems

D-Bus offers two types of communication buses, the system and the session bus. By default and in most setups, MPRIS is expected to run on the session bus (for example, `playerctl` requires that). However, in headless contexts, the session bus is usually not available.

In this case, you basically have two options:

## Option 1: Launch with `dbus-launch`

By creating a wrapper script and using `dbus-launch`, `spotifyd` can be run with its own bus. This could look like the following

`./spotify_wrapper.sh`:
```bash
#!/bin/bash

echo "$DBUS_SESSION_BUS_ADDRESS" > /tmp/spotifyd_bus
echo "To use spotifyd's session bus, run 'export DBUS_SESSION_BUS_ADDRESS=$(cat /tmp/spotifyd_bus)'"
spotifyd --no-daemon --use-mpris
```

Then, execute this script using `dbus-launch ./spotify_wrapper.sh` and follow the instructions in the output.

## Option 2: Using the system bus

Instead of creating a new session bus, we can instead make use of the always existing system bus.
To instruct `spotifyd` to use the system instead of the session bus, set the `--dbus-type system` cli flag / `dbus_type = "system"` config value.

By default, requesting names on the system bus requires special priveleges, which `spotifyd` doesn't have. So, unless being run as root, this will fail. To allow a non-root user to request the `spotifyd` name, we need to create the following file (replacing `your user` by something sensible):

`/usr/share/dbus-1/system.d/spotifyd.conf`:
```xml
<!DOCTYPE busconfig PUBLIC
          "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
          "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <!-- Only this user can own the spotifyd interfaces -->
  <policy user="your user">
    <allow own_prefix="rs.spotifyd"/>
    <allow own_prefix="org.mpris.MediaPlayer2.spotifyd"/>
  </policy>

  <!-- Allow this user, to invoke methods on these two interfaces -->
  <policy user="your user">
    <allow send_destination_prefix="rs.spotifyd"/>
    <allow send_destination_prefix="org.mpris.MediaPlayer2.spotifyd"/>
  </policy>
</busconfig>
```

Make sure to reload the D-Bus configuration with `systemctl reload dbus`.

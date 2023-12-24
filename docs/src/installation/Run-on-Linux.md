# Raspberry Pi install guide

This guide will help you to install `spotifyd` on a Linux system and have it always running. This guide requires a spotifyd executable wiht full path: /usr/bin/spotifyd.
See specific system installation guides or the [build guide](Build-on-Linux.md) how to set this up.

## Configuring spotifyd

Spotifyd comes pre-configured with defaults that should be working in most cases, but if you want to tweak it further to your needs, have a look at the [configuration section](../config/File.md) of this book.

## Start spotifyd from CLI

Simply run:

```bash
/usr/bin/spotifyd --no-daemon
```

or if you want to run it in the background:

```bash
/usr/bin/spotifyd
```

Without the --no-daemon argument the process forks itself into the background.

## Start spotifyd as service / on boot

For systemd setup see [Systemd](../config/services/Systemd.md)

Now see if you can find it in the normal Spotify client (Devices in right bottom corner). Retry the above steps if you can't find it.

## Starting with DBus enabled on boot

If you have configured spotifyd to start at boot as system wider service or as user service with linger and if you have DBus enabled to listen to player changes like
track changes or if you want to control the player by other process, you need to to configure spotifyd to use the system DBus as it can't register services on the session dbus at boot.
You need to have a config file and configure to use the system dbus, see `Configuring spotifyd`, you can delete everything you don't need to use the standard values.
But you need to configure 2 values:

```config
use_mpris = true
dbus_type = "system"
```

In the standard DBus config, no one is allowed to register services on the system dbus, so you need to configure it.
Create a configuration file under `/usr/share/dbus-1/system.d/`, eg. `spotifyd-dbus.conf` (must end with `.conf`).

```bash
sudo nano /usr/share/dbus-1/system.d/spotifyd-dbus.conf
```

Add the following content:

As system wide service:

```content
<!DOCTYPE busconfig PUBLIC
 "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">

<busconfig>
    <policy user="user">
        <allow eavesdrop="true"/>
        <allow eavesdrop="true" send_destination="*"/>
    </policy>
    <policy user="root">
        <allow own_prefix="org.mpris.MediaPlayer2.spotifyd"/>
        <allow send_type="method_call" log="true"/>
    </policy>
    <policy context="default">
    </policy>
</busconfig>
```

User is the user account name with which you listen to the DBUs messages and root the account name wich runs the service.

As user service the account name with which you run the service and with which you listen to the DBUs messages may be the same, then the config would be:

```content
<!DOCTYPE busconfig PUBLIC
 "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">

<busconfig>
    <policy user="user">
        <allow eavesdrop="true"/>
        <allow eavesdrop="true" send_destination="*"/>
        <allow own_prefix="org.mpris.MediaPlayer2.spotifyd"/>
        <allow send_type="method_call" log="true"/>
    </policy>
    <policy context="default">
    </policy>
</busconfig>
```

## Using other audio backends like PulseAudio
It may be usefull to not use the ALSA interface but Pulse, e.g. if several audio sources want to acces the sound card, it is likely that the sound card
doesn't support it, then you need to use the PulseAudio interface.

Make sure spotifyd is compiled with PulseAudio support. You need to have a config file and configure 1 value:

```config
backend = "pulseaudio"
```

Latest Raspberry OS versions (Bookworm) run pipewire with PulseAudio interface as default.

## Known issues
- As user service, spotifyd crashes once at startup with Message: called 'Result::unwrap()' on an 'Err' value: DnsSdError(Os { code: 19, kind: Uncategorized, message: "No such device" }).
  Be sure to configure your service file with 

  ```
  [Service]
  ExecStart=/usr/bin/spotifyd --no-daemon
  Restart=always
  RestartSec=12
  ```

  so that it restarts after the crash.

- As system daemon with PulseAudio backend starting the service will result in a connection refused error from PulseAudio.
# Raspberry Pi install guide

This guide will help you to install `spotifyd` on a Raspberry Pi and have it always running.

## Download

1. Download the latest ARMv6 from <https://github.com/Spotifyd/spotifyd/releases> (use `wget`)
2. Unzip the file: `tar xzf spotifyd-linux-arm6*`
You will now see a file called `spotifyd`. You can run it with `./spotifyd --no-daemon`
The binaries on the download site are all 32 bit binaries, you cannot easily run them on a current 64-bit Raspberry OS.
Trying to run them will result in 'cannot execute: required file not found'
To run on a 64 bit Raspberry OS, see `Raspberry-Pi-64.md`.

It is recommended to copy the file to usr/bin, so that everyone can run it or use it for a daemon:

```bash
sudo cp ./spotifyd /usr/bin
```

## Systemd daemon file

Create a systemd service file and copy the [default configuration](https://github.com/Spotifyd/spotifyd/blob/master/contrib/spotifyd.service) into it. Change **ExecStart** to where you unzipped the `spotifyd` binary.

```bash
sudo nano /etc/systemd/system/spotifyd.service
```

If you want to run as user instead of root or have some `Failed to get D-Bus connection: Connection refused`, you define `spotifyd.service` in your home directory:

```bash
mkdir -p ~/.config/systemd/user/
nano ~/.config/systemd/user/spotifyd.service
systemctl --user daemon-reload
```

Or in the user diretcory of systemd:

```bash
sudo nano /etc/systemd/user/spotifyd.service
```

## Configuring spotifyd

Spotifyd comes pre-configured with defaults that should be working in most cases, but if you want to tweak it further to your needs, have a look at File.md at the [configuration section](../config/) of this book.

## Start the service

As user daemon

```bash
systemctl --user start spotifyd.service
```

As system daemon

```bash
sudo systemctl start spotifyd.service
```

Now see if you can find it in the normal Spotify client (Devices in right bottom corner). Retry the above steps if you can't find it.

## Starting spotifyd at boot

As system daemon:

```bash
sudo systemctl enable spotifyd.service
```

As user daemon is not recommended, it crashes currently with Message: called 'Result::unwrap()' on an 'Err' value: DnsSdError(Os { code: 19, kind: Uncategorized, message: "No such device" }):

```bash
sudo loginctl enable-linger <username>
systemctl --user enable spotifyd.service
```

The first command is required to enable your user to run long-running services. Without it `systemd` would kill the `spotifyd` process as soon as you log out, and only run it when you log in.

Now `spotifyd` is always running on the Pi, so you can use it as a listening device remotely!

## Starting with dbus enabled at boot

If you have dbus enabled, you need to to configure spotifyd to use the system dbus as it can't register services on the session dbus at boot.
You need to have a config file and configure to use the system dbus, see `Configuring spotifyd`, you can delete everything you don't need to use the standard values. But you need to configure 2 values:

```config
use_mpris = true
dbus_type = "system"
```

In the standard config, no one is allowed to register services on the system dbus, so you need to configure it. Create a configuration file under /usr/share/dbus-1/system.d/, eg. spotifyd-dbus.conf (must end with .conf)

```bash
sudo nano /usr/share/dbus-1/system.d/spotifyd-dbus.conf
```

Add the following content:

```content
<!DOCTYPE busconfig PUBLIC
 "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">

<busconfig>
    <policy user="root">
        <allow own_prefix="org.mpris.MediaPlayer2.spotifyd"/>
        <allow send_type="method_call" log="true"/>
    </policy>
    <policy context="default">
    </policy>
</busconfig>
```

If you try to run as user, replace root with the user account name.

## Known issues / Logging
Logging in files is currently not possible, the daemon crashes without the --no-daemon argument, which redirects the output to stdout. Without the parameter it tries to
output at syslog, but even with syslog installed it crashed.
To get ist least the latest output run

```bash
sudo systemctl status spotifyd.service
```
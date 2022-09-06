# Raspberry Pi install guide

This guide will help you to install `spotifyd` on a Raspberry Pi and have it always running.

## Download

1. Download the latest ARMv6 from https://github.com/Spotifyd/spotifyd/releases (use `wget`)
2. Unzip the file: `unzip spotifyd-*.zip`
You will now see a file called `spotifyd`. You can run it with `./spotifyd --no-daemon`

## Systemd daemon file

Create a systemd service file and copy the [default configuration](https://github.com/Spotifyd/spotifyd/blob/master/contrib/spotifyd.service) into it. Change **ExecStart** to where you unzipped the `spotifyd` binary.

```bash
sudo nano /etc/systemd/user/spotifyd.service
```

if you want to run as user instead of root or have some `Failed to get D-Bus connection: Connection refused`, you define `spotifyd.service` in your home directory:

```bash
mkdir -p ~/.config/systemd/user/
nano ~/.config/systemd/user/spotifyd.service
systemctl --user daemon-reload
```

## Configuring spotifyd

Spotifyd comes pre-configured with defaults that should be working in most cases, but if you want to tweak it further to your needs, have a look at the [configuration section](../config/) of this book.

## Start the service

```bash
systemctl --user start spotifyd.service
```

Now see if you can find it in the normal Spotify client (Devices in right bottom corner). Retry the above steps if you can't find it.

## Starting spotifyd at boot

```bash
sudo loginctl enable-linger <username>
systemctl --user enable spotifyd.service
```

The first command is required to enable your user to run long-running services. Without it `systemd` would kill the `spotifyd` process as soon as you log out, and only run it when you log in.
Now `spotifyd` is always running on the Pi, so you can use it as a listening device remotely!

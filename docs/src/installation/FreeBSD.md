# FreeBSD install guide

This guide will help you to install `spotifyd` on FreeBSD and have it always running.

`spotifyd` is available for the FreeBSD architectures :

* amd64
* i386
* arm64

## Install

```sh
sudo pkg install spotifyd
```

## Config file


Input the following data. Change the **username**, **password**, **device_name**, and the **bitrate**.

```toml
[global]
username = "USER"
password = "PASS"
backend = "portaudio"
device = "/dev/dsp"
#onevent = command_run_on_playback_event
device_name = "name_in_spotify_connect"
bitrate = 96|160|320
cache_path = "cache_directory"
volume-normalisation = true
normalisation-pregain = -10
```

## Start the service

```bash
sudo service spotifyd onestart
```

Now see if you can find it in the normal Spotify client (Devices in right bottom corner). Retry the above steps if you can't find it.

## Starting spotifyd at boot

```sh
sudo sysrc spotifyd_enable=YES
```
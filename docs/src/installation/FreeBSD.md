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

## Configuring spotifyd

If you installed spotifyd using the above method, you'll either need to supply `--backend portaudio` as a command-line argument or add `backend = "portaudio"` to `/usr/local/etc/spotifyd.conf`.

Apart from that, spotifyd comes pre-configured with defaults that should be working in most cases, but if you want to tweak it further to your needs, have a look at the [configuration section](../config/) of this book.

## Start the service

```bash
sudo service spotifyd onestart
```

Now see if you can find it in the normal Spotify client (Devices in right bottom corner). Retry the above steps if you can't find it.

## Starting spotifyd at boot

```sh
sudo sysrc spotifyd_enable=YES
```

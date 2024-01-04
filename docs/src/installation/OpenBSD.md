# OpenBSD install guide

`spotifyd` is available on all supported Rust architectures:

* aarch64
* amd64
* i386
* powerpc64
* riscv64
* sparc64

## Install

```sh
# pkg_add spotifyd
```

## Configuring spotifyd

The official package uses PortAudio and works out of the box, no configuration is required.

## Running spotifyd

You may start `spotifyd` as background daemon in your `~/.xsession` X11 startup script
or have clients like `spotify-qt` start/stop it accordingly.

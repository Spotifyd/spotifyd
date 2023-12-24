# Raspberry Pi install guide

This guide will help you to install `spotifyd` on a Raspberry Pi and have it always running.

## Download

1. Download the latest ARMv6 from <https://github.com/Spotifyd/spotifyd/releases> (use `wget`)
2. Unzip the file: `tar xzf spotifyd-linux-arm6*`
You will now see a file called `spotifyd`. You can run it with `./spotifyd --no-daemon`
The ARM binaries on the download site are all 32 bit binaries, you cannot easily run them on an OS with ARM64 only architecture like Raspberry Pi OS.
Trying to run them will result in `cannot execute: required file not found`.
To run on a ARM64 only architecture, have a look at [Raspberry-Pi-64_ARM64](Raspberry-Pi-64_ARM64.md).

It is recommended to copy the file to usr/bin, so that everyone can run it or use it for a service:

```bash
sudo cp ./spotifyd /usr/bin
```

For further configuration see [Run-on-Linux](Run-on-Linux.md)
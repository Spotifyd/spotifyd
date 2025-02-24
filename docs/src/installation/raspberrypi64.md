# Installation on RaspberryPi OS 64-bit

Unfortunately, we do not yet provide 64-bit binaries for ARM.
Trying to run them will result in `cannot execute: required file not found`.  

To run spotifyd on a 64-bit Raspberry Pi OS, you have two possiblities. Build the 64-bit binary by yourself or add the 32-bit architecture as an additional architecture to your 64-bit Raspberry Pi OS.

## Option 1: Building yourself

To build `spotifyd` yourself, head over to [Building from source](./source.md). Note, however, that building can take a long time, especially on low-power devices like a RaspberryPi.

## Option 2: Add the 32-bit architecture

### Adding architecture and dependency packages

These commands add the architecture and install the required packages for the architecure:

```bash
dpkg --add-architecture armhf
sudo apt update
sudo apt install libasound2-plugins:armhf
```

Now you can go on with the [regular install guide](./), by assuming the `armhf` architecture.  

Downloading other variants than thin like full or default may require further armhf packages to be installed with the command like above:

```console
sudo apt install packagename:armhf
```

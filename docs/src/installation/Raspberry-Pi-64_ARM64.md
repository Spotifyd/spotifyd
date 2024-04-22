# 64 bit Raspery OS build guide

The ARM binaries on the download site are all 32 bit binaries, you cannot easily run them on an OS with ARM64 only architecture like Raspberry Pi OS.  
Trying to run them will result in `cannot execute: required file not found`.  

To run spotifyd on a 64 bit Raspberry Pi OS, you have two possiblities. Build the 64 bit binary by yourself or add the 32 bit architecture as  
an additional architecture to your 64 bit Raspberry Pi OS.

Let's start with building the 64 bit version, as it offers you more possibilities like adding the DBus feature for controlling the service with  
DBus commands.

## Option 1: Building the 64 bit version

See [Build-on-Linux.md](Build-on-Linux.md)

## Option 2: Add the 32 bit architecture

### Adding architecture and dependency packages

These commands add the architecture and install the required packages for the architecure:

```bash
dpkg --add-architecture armhf
sudo apt update
sudo apt install libasound2-plugins:armhf
```

Now you can go on with the [32-bit instructions](Raspberry-Pi-32_ARM32.md), but download the `armhf-slim.tar.gz` file instead of the armv6 file.  
The other instructions are the same once you have the binary running.  
Downloading other variants like full or default may require further armhf packages to be installed with the command like above:

```bash
sudo apt install packagename:armhf
```

## Run
Now you can go on with the instructions in [Run-on-Linux](Run-on-Linux.md).

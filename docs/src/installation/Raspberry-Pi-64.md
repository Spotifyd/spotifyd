# 64 bit Raspery OS build guide

To run spotifyd on a 64 bit Raspberry Pi OS, you have two possiblities. Compile the 64 bit binary by yourself or add the 32 bit architecture as 
an additional architecture to your 64 bit Raspberry Pi OS.

Let's start with compiling the 64 bit version, as it offers you more possibilities like adding the DBus feature for controlling the service with
DBus commands.

## Option 1: Compiling the 64 bit version

### Uninstall the preinstalled rust compiler

spotifyd may require a newer rust toolchain than the one which is delivered with Raspberry Pi OS.

```bash
sudo apt remove rustc
```

### Install the rust toolchain

To install the latest rust toolchain, follow the installation instructions on [rustup.rs][rustup].

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Chose simply option 1.

After installation finished, enter

```bash
source "$HOME/.cargo/env"
```

### Clone the repository

E.g. in your Downloads Directory. If it does not exist, create it or do it somewhere else.

```bash
cd /home/user/Downloads
git clone https://github.com/Spotifyd/spotifyd.git
```

### Building spotifyd

This takes a while...

```bash
cd spotifyd
cargo build --release
```

This will build a basic version, but you can add additional features like the mentioned DBus or other sound backends by adding additional flags.

E.g. with DBus support

```bash
cd spotifyd
cargo build --release --features dbus_mpris
```

The resulting binary will be placed in `./target/release/spotifyd` and you can run it using `./target/release/spotifyd`.
Now you can go on with the instructions in `Raspberry-Pi-32.md`, the other instructions are the same once you have the binary running.

## Option 2: Add the 32 bit architecture

### Adding architecture and dependency packages

These commands add the architecture and install the required packages for the architecure:

```bash
dpkg --add-architecture armhf
sudo apt update
sudo apt install libasound2-plugins:armhf
```

Now you can go on with the [32-bit instructions](Raspberry-Pi-32.md), but download the `armhf-slim.tar.gz` file instead of the armv6 file.
The other instructions are the same once you have the binary running.
Downloading other variants like full or default may require further armhf packages to be installed with the command like above:

```bash
sudo apt install packagename:armhf
```


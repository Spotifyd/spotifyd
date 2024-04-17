# Installation

## Provided binaries

We provide pre-built binaries through GitHub Actions for the more popular platforms: Linux, macOS and ARMv7. You can find them [here](https://github.com/Spotifyd/spotifyd/releases). For extra integrity, the file's SHA-512 gets calculated and uploaded as well.

The provided binaries come in two flavours, `slim` and `full`. Each are compiled with different features. `slim` only contains the platform's most used audio backend, `full` has also all optional features enabled (see [Feature Flags](#feature-flags)).

## Provided packages

There are packages for the following systems:

- [Arch Linux (via the Extra repository)](https://archlinux.org/packages/extra/x86_64/spotifyd/)
- [MacOS (via homebrew)](./MacOS.md)

## Building from source

You can also compile `Spotifyd` yourself, allowing you to make use of feature flags. `Spotifyd` is written in Rust. You can download the toolchain (compiler and package manager) over at [rustup.rs](https://rustup.rs). Follow their instructions to get started.

> __Note:__ Please make sure that you compile the package using the most recent `stable` version of Rust available through `rustup`. Some distro versions are quite outdated and might result in compilation errors.

`Spotifyd` might require additional libraries during build and runtime, depending on your platform and the way to compile it (static or dynamic). The following table shows the libraries needed for each OS respectively.

| Target Platform | Libraries                                            |
|-----------------|------------------------------------------------------|
| Fedora          | alsa-lib-devel make gcc                              |
| openSUSE        | alsa-devel make gcc                                  |
| Debian          | libasound2-dev libssl-dev libpulse-dev libdbus-1-dev |
| Arch            | base-devel alsa-lib libogg libpulse dbus             |
| macOS           | dbus pkg-config portaudio                            |

> __Note:__ The package names for Linux are the ones used on Debian based distributions (like Ubuntu). You will need to adapt the packages for your distribution respectively.

You can find more details about building on Linux [here](Build-on-Linux.md).

To compile the binary, run

```bash
cargo build --release
```

To install the resulting binary, run

```bash
cargo install --path . --locked
```

### Installing with Cargo

If you have `cargo` installed, you can directly install `spotifyd` by running:

```bash
cargo install spotifyd --locked
```

That will compile and install `spotifyd`'s latest version under `$HOME/.cargo/bin` for you.

### Building a Debian package

You can use the `cargo-deb` create in order to build a Debian package from source.
Install it by:

```bash
cargo install cargo-deb
```

Then you can build and install the Debian package with:

```bash
cargo deb --install
```

Note, that when building a Debian package, the `--release` is passed to the
build command already and you do not need to specify it yourself.  See for the
flags that are set by default in `Cargo.toml`.

### Feature Flags

`Spotifyd` is split into a base package plus additional features that can be toggled on or off during compilation. Those can be split into two groups: The audio backend features that are responsible for playing back the music and additional functionality features, which enhance your experience using `spotifyd`.

To enable an additional audio backend, pass `<audio_backend_name>_backend` as a feature flag. We currently support `alsa`, `pulseaudio` and `portaudio`.

`Spotifyd` provides the following additional functionality:

| Feature Flag | Description                                                                         |
|--------------|-------------------------------------------------------------------------------------|
| dbus_keyring | Provides password authentication over the system's keyring (supports all platforms) |
| dbus_mpris   | Provides multimedia key support (Linux only)                                      |

> __Note:__ Compiling Spotifyd with all features and the pulseaudio backend on Ubuntu would result in the following command: `cargo build --release --no-default-features --features pulseaudio_backend,dbus_keyring,dbus_mpris`

# Building from source

The guide below assumes that you're building `spotifyd` on the system that you want to run it on. If you'd instead prefer to cross-compile, head over to [this section](./cross-compilation.md).

You can also compile `spotifyd` yourself, allowing you to tailor it perfectly to your needs or get the latest fixes. `spotifyd` is written in Rust. You can download the toolchain (compiler and package manager) over at [rustup.rs](https://rustup.rs). Follow their instructions to get started.

> __Note:__ Please make sure that you compile the package using the most recent `stable` version of Rust available through `rustup`. Some distro versions are quite outdated and might result in compilation errors.

## Required packages

`spotifyd` might require additional libraries during build and runtime, depending on your platform and the way to compile it (static or dynamic). The following table shows the libraries needed for each OS respectively.

| Target Platform | Libraries                                            |
|-----------------|------------------------------------------------------|
| Fedora          | alsa-lib-devel make gcc                              |
| openSUSE        | alsa-devel make gcc                                  |
| Debian          | libasound2-dev libssl-dev libpulse-dev libdbus-1-dev |
| Arch            | base-devel alsa-lib libogg libpulse dbus             |
| macOS           | dbus pkg-config portaudio                            |

If you're building on a non-standard target, for example the RaspberryPi, you might need an additional `libclang-dev` and `cmake` package for one of our dependencies. Details can be found on [this page](https://aws.github.io/aws-lc-rs/requirements/linux.html).

## Installing with cargo

To build and install the latest version of `spotifyd`, you can use the package manager for rust. The base command is the following

```console
cargo install spotifyd --locked
```

If you would rather install from the latest commit on GitHub, download or clone the project, enter the directory and run

```console
cargo install --path . --locked
```

> __Note:__ Both methods will install the binary in `$XDG_DATA_HOME/cargo/bin` or `$HOME/.cargo/bin`. To execute it, you must make sure that this location is part of your shell's `$PATH` variable. Also, you might have to change the paths when following other parts of this documentation.

## Compiling with cargo

To just build the binary without installing, run

```console
cargo build --release --locked
```

This will build the binary and leave the result at `./target/release/spotifyd`.

## Building a Debian package

You can use the `cargo-deb` crate in order to build a Debian package from source.
Install it by:

```console
cargo install cargo-deb
```

Then you can build and install the Debian package with:

```console
cargo deb --install
```

Note, that when building a Debian package, the `--release` is passed to the
build command already and you do not need to specify it yourself.  See for the
flags that are set by default in `Cargo.toml`.

## Feature Flags

`spotifyd` is split into a base package plus additional features that can be toggled on or off during compilation. Those can be split into two groups: The audio backend features that are responsible for playing back the music and additional functionality features, which enhance your experience using `spotifyd`.

| Feature Flag | Description                                                                         |
|--------------|-------------------------------------------------------------------------------------|
| `alsa_backend` | Provides support for the ALSA backend. Should work in most setups and is enabled by default. |
| `pulseaudio_backend` | Support for PulseAudio. |
| `rodio_backend` | Rust-native implementation of audio backends on all platforms. Does not need any external packages to be installed. |
| `portaudio_backend` | Audio backend that can be used on non-Linux systems. |
| `rodiojack_backend` | Support for the Jack backend. |
| `dbus_mpris`   | Provides multimedia key support (Linux and BSD only)                                |

To customize your build, pass a subset of the features listed above to any of the `cargo` commands above via `--features <feature1>,<feature2>,...`. Disable the default feature `alsa_backend` with `--no-default-features`. So an example command could look like the following:

```
cargo install spotifyd --locked --no-default-features --features rodio_backend,dbus_mpris
```

# Ubuntu install guide

## Install the rust toolchain

To install the latest rust toolchain, follow the installation instructions on [rustup.rs][rustup].

> **Note:** If you installed rust before via apt, you need to remove it before installing rustup.
> We recommend to always use the latest version and don't guarantee compatibility with older ones.

## Install the requirements

```bash
sudo apt install libasound2-dev libssl-dev pkg-config
```

## Clone the repository

```bash
git clone https://github.com/Spotifyd/spotifyd.git
```

## Building spotifyd

This takes a while...

```bash
cd spotifyd
cargo build --release
```

The resulting binary will be placed in `target/release/spotifyd`

## Running spotifyd

You can run it using `./target/release/spotifyd`

[rustup]: https://rustup.rs/

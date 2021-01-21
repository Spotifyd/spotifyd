# Ubuntu install guide

## Install the requirements

```bash
sudo apt install rustc cargo libasound2-dev libssl-dev pkg-config
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

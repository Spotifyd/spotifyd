# Fedora installation guide

## Install the requirements (not including Rust)

`sudo dnf install portaudio-devel`

## Clone the repository

`git clone https://github.com/Spotifyd/spotifyd.git`

## Building spotifyd

```bash
cd spotifyd
cargo build --release
```

The resulting binary will be placed in `target/release/spotifyd`

## Running spotifyd

You can run it using `./target/release/spotifyd`
# Contributing

## Setup

__Important:__ Make sure to install the provided git hooks by running `setup-dev-workspace.sh`! 

### Install Rust

It is recommended to install rust through [rustup](https://rustup.rs). System package managers often have outdated versions of Rust or don't include rustfmt/clippy.

### Install rustfmt and clippy

```sh
rustup component add rustfmt
rustup component add clippy
```

### External Dependencies

The rodio backend requires alsa development libraries on linux. These should be available through your package manager. No extra dependencies should be needed on macOS/Windows.

Other backends will need additional dependencies. Some are documented in the (readme)[https://github.com/Spotifyd/spotifyd/blob/master/README.md] and the (librespot repo)[https://github.com/librespot-org/librespot/blob/dev/CONTRIBUTING.md].

## Building and Running the project

To run spotifyd with the rodio backend run

```sh
cargo run --features rodio_backend --no-default-features  -- --backend rodio --no-daemon
```

See the readme for information on setting up a config file and what features are available.

## Contributing Code

Note that spotifyd uses `Cargo.lock` to track dependency versions. It shouldn't be changed unless you are intentionally bumping a dependency version.

Check your code with `rustfmt` and `clippy`.

```sh
cargo fmt -- --check
cargo clippy --no-default-features --features rodio_backend -- -D warnings
```

CI will run `clippy --all-targets --all-features -- -D warnings` but this requires having dependencies for all features installed.

Create a PR on github.

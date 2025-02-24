# Installation

Getting `spotifyd` on your system should be as easy as downloading a binary in most cases.
If you'd like to learn how to compile `spotifyd` yourself, head over to [building from source](./source.md). And if you're running a 64-bit RaspbianOS system, have a look [here](./raspbian64.md).

## Linux

Some linux distributions include `spotifyd` in their official repositories. Have a look at [Repology](https://repology.org/project/spotifyd/versions)
for a list of the distributions that currently ship an up-to-date version of `spotifyd`.

If your distribution is not supported or the provided version is too old, skip to [this section](#installing-from-releases) in order to install one of our pre-built binaries.

## macOS

If you're a homebrew user, installing `spotifyd` is as easy as running

```console
brew install spotifyd
```

## Installing from releases

If none of the above methods work for you, you can also use our provided binaries.

First, you need to find a suitable binary for your platform. The provided binaries differ in the available features
and the platform architecture that they were built for. You can find the latest binaries [here](https://github.com/Spotifyd/spotifyd/releases).

**Feature Sets:**

- `full`: **all audio backends** and **MPRIS** support
- `default`: **one audio backend** (depending on your platform: PulseAudio, PortAudio, ALSA) and **MPRIS** support
- `slim`: **one audio backend** (depending on your platform) and **no MPRIS** support (good for headless systems)

If you're unsure which version to choose, just go for `default` on desktop systems and `slim` on headless systems.

**Architecture:**

If you're on Linux, check your platform architecture with `uname -m`:

- `x86_64`: Download one of the `spotifyd-linux-{full,default,slim}.tar.gz` packages.
- `armhf`, `armv7`: Download one of the `spotifyd-linux-armhf-{full,default,slim}.tar.gz` packages.
- `armv6`: Download the `spotifyd-linux-armv6-slim.tar.gz` package.
- `aarch64`: Head over to the [RaspberryPi OS 64-bit installation guide](./raspberrypi64.md).

If you're on macOS, download one of the `spotifyd-macos-{full,default,slim}.tar.gz` packages.

You should now extract the downloaded archive, make the `spotifyd` file executable and copy it to a sensible location. This can be done using the following commands:

```console
$ tar xzf spotifyd-*.tar.gz # extract
$ cd spotifyd-*/
$ chmod +x spotifyd # make binary executable
$ # move to correct location, e.g. on Linux:
$ # for a user-wide installation (make sure that your $PATH includes ~/.local/bin)
$ mv spotifyd ~/.local/bin/spotifyd
$ # for a system-wide installation
$ sudo chown root:root spotifyd
$ sudo mv spotifyd /usr/local/bin/spotifyd
```

## Running

Now that you have installed `spotifyd`, you can check if everything was successful by running `spotifyd --version`.

You should be ready to go now and after running `spotifyd --no-daemon`, it should appear in an **official** Spotify client which is on the same network.
If this does not work, you can head over to the [troubleshooting section](../troubleshooting.md) or look at [different methods of authentication](../configuration/auth.md).

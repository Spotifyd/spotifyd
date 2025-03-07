# Cross-Compilation

If you want to run `spotifyd` on lower-power hardware such as a RaspberryPi, but none of our prebuilt binaries suit your needs, you might want to cross-compile `spotifyd` on a more powerful machine and deploy the binary on the target system.

## Using `cross`

The easiest way to cross-compile is using the amazing [`cross` project](https://github.com/cross-rs/cross). This way, the build environment comes already pre-configured.
Follow the instructions in their README to install `cross`.

<div class="warning">

In the current latest release of cross (v0.2.5), some targets are too outdated and compilation will fail. Thus, it is currently recommended to install the latest version of `cross` from their git repo instead of a prebuilt binary.

</div>

Then, you should be able to run `cross build --target <your desired target>`, where target is one of the targets in `rustc --print target-list`. Please also refer to [the general from source guide](./source.md) for additional flags that you might want to append to that command.

If this was successful, copy the resulting binary from `target/<your desired target>/{release,debug}/spotifyd` to the `spotifyd` machine and try running it there.

If `cross` doesn't support your target, you can try the alternative approach using Docker and QEMU below.

If compilation of your target isn't working even though `cross` supports it, feel free to open an issue on our GitHub or join the [community matrix channel](https://matrix.to/#/#spotifyd:matrix.org) and ask there.

## Using Docker and QEMU

We can also use `docker` to cross compile on every platform and OS that runs `docker` and `qemu`:

1. Setup a docker [custom builder](https://docs.docker.com/build/building/multi-platform/#create-a-custom-builder)

    ```shell
    docker buildx create \
      --name container-builder \
      --driver docker-container \
      --use --bootstrap
    ```

    If you are **not** using Docker-Desktop you might have to install [QEMU](https://docs.docker.com/build/building/multi-platform/#install-qemu-manually)

2. Create a docker `docker-compose.yml`

    Here we are building a `arm64` binary, so we set `platform: linux/arm64`

    ```yaml
    services:
      build-container:
        image: rust:1-bookworm
        platform: linux/arm64
        command: bash -c "
            apt-get update &&
            apt-get install -y \
              libasound2-dev \
              libssl-dev \
              jq \
              pkg-config &&
            wget -O - https://api.github.com/repos/Spotifyd/spotifyd/tarball/$(\
                curl -SsL https://api.github.com/repos/Spotifyd/spotifyd/releases/latest \
                  | jq '.tag_name' -r) \
              | tar xzv -C /spotifyd --strip-components=1 &&
            cargo build --release &&
            cp /spotifyd/target/release/spotifyd /build/"
        working_dir: /spotifyd
        volumes:
          - ./:/build
    ```

3. Run `docker compose up`

    This will copy the build `spotifyd` binary in the current directory.

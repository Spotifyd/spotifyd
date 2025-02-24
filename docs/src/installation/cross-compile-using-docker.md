# Cross Compilation using Docker

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

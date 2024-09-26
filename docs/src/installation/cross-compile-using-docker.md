We can also use `docker` to cross compile on every platform and OS that runs `docker` and `qemu`:

1. Setup a docker [custom builder](https://docs.docker.com/build/building/multi-platform/#create-a-custom-builder)
```shell
docker buildx create \
  --name container-builder \
  --driver docker-container \
  --use --bootstrap
```

2. Create a docker `compose-file.yml`:
```yaml
services:
  build-container:
    image: rust:1.79-bookworm
    platform: linux/arm64
    command: bash -c "
        apt-get update &&
        apt-get install -y \
          libasound2-dev \
          libssl-dev \
          pkg-config &&
        curl -sSL https://api.github.com/repos/Spotifyd/spotifyd/tarball/v0.3.5 | tar xz -C /spotifyd --strip-components=1 &&
        cargo build --release &&
        cp /spotifyd/target/release/spotifyd /build/"
    working_dir: /spotifyd
    volumes:
      - ./:/build
```

3. Run `docker compose up`

This will copy the build `spotifyd` binary in the current directory. 

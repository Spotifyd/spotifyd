FROM dlecan/rust-crosscompiler-arm:nightly

RUN apt-get update
RUN apt-get install -qq -y libasound2-dev:armhf pkg-config

VOLUME ["/source"]
WORKDIR /source

ENV PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig/

FROM dlecan/rust-crosscompiler-arm:stable

RUN apt-get update
RUN apt-get install -qq -y libasound2-dev:armhf pkg-config

VOLUME ["/source"]
WORKDIR /source

ENV PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig/
ENV OBJCOPY $CC_DIR/arm-linux-gnueabihf-objcopy

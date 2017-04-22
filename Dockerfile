FROM ekidd/rust-musl-builder

RUN sudo apt-get update && sudo apt-get install -qq -y gcc-arm-linux-gnueabihf libasound2-dev linux-libc-dev && VERS=1.1.3 && \
    rustup target add armv7-unknown-linux-gnueabihf && \
    curl -LO http://archive.raspbian.org/raspbian/dists/wheezy/main/binary-armhf/Packages && \
    curl -LO http://archive.raspbian.org/raspbian/`grep 'libasound2_.*\.deb' Packages | cut -d' ' -f2` && \
    ar x libasound2* && tar xf data.tar.gz && \
    sudo mv usr/lib/arm-linux-gnueabihf/ /usr/lib/ && \
    sudo ln -s /usr/lib/arm-linux-gnueabihf/libasound.so.2.0.0 /usr/lib/arm-linux-gnueabihf/libasound.so && \
    (echo "[target.armv7-unknown-linux-gnueabihf]"; echo "linker = \"arm-linux-gnueabihf-gcc\"") | sudo tee --append ~/.cargo/config

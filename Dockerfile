FROM ekidd/rust-musl-builder

RUN sudo apt-get update && sudo apt-get install -qq -y gcc-arm-linux-gnueabihf linux-libc-dev && VERS=1.1.3 && \
    sudo ln -s /usr/include/asm-generic/ /usr/include/asm && \
    cd /home/rust/libs && \
    curl -LO ftp://ftp.alsa-project.org/pub/lib/alsa-lib-$VERS.tar.bz2 && \
    curl -LO https://bitbucket.org/GregorR/musl-cross/downloads/crossx86-arm-linux-musleabi-0.9.11.tar.xz && \
    tar xf crossx86-arm-linux-musleabi-0.9.11.tar.xz && \
    tar xf alsa-lib-$VERS.tar.bz2 && cp -r alsa-lib-$VERS alsa-lib-arm && cd alsa-lib-$VERS && \
    sed -i 's,#if !defined(_POSIX_C_SOURCE) && !defined(_POSIX_SOURCE),#if 0,' include/global.h && \
    sed -i 's,sys/poll\.h,poll.h,' include/asoundlib-head.h && \
    sed -i 's,sys/poll\.h,poll.h,' include/local.h && \
    env CFLAGS="-include stdlib.h -D_POSIX_C_SOURCE=200809L -D_GNU_SOURCE -include limits.h" \
    CC=musl-gcc ./configure --enable-static --enable-shared=no --disable-python --prefix=/usr --with-config-dir=/usr/share/alsa/ && \
    env C_INCLUDE_PATH=/usr/include/  make && sudo make install && \
    cd .. && cd alsa-lib-arm && \
    env CC=/home/rust/libs/arm-linux-musleabi/bin/arm-linux-musleabi-gcc ./configure --enable-static --enable-shared=no \
    --disable-python --prefix=/usr --host x86_64-unknown-linux-gnu && \
    rustup target add armv7-unknown-linux-gnueabihf && \
    curl -LO http://archive.raspbian.org/raspbian/dists/wheezy/main/binary-armhf/Packages && \
    curl -LO http://archive.raspbian.org/raspbian/`grep 'libasound2_.*\.deb' Packages | cut -d' ' -f2` && \
    ar x libasound2* && tar xf data.tar.gz && \
    sudo mv usr/lib/arm-linux-gnueabihf/ /usr/lib/ && \
    sudo ln -s /usr/lib/arm-linux-gnueabihf/libasound.so.2.0.0 /usr/lib/arm-linux-gnueabihf/libasound.so && \
    (echo "[target.armv7-unknown-linux-gnueabihf]"; echo "linker = \"arm-linux-gnueabihf-gcc\"") | sudo tee --append ~/.cargo/config

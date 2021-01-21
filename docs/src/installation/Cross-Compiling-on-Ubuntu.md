# Ubuntu compilation guide

These are some notes on cross-compiling, that worked on WSL xenial Ubuntu.

### Activate the cross compilation in dpkg

```bash
dpkg --add-architecture armhf
sed 's/deb http/deb \[arch=amd64,i386\] http/' -i /etc/apt/sources.list
echo >/etc/apt/sources.list <<EOF
deb [arch=armhf] http://ports.ubuntu.com/ $(lsb_release -cs) main universe restricted multiverse
deb [arch=armhf] http://ports.ubuntu.com/ $(lsb_release -cs)-updates main universe restricted multiverse
deb [arch=armhf] http://ports.ubuntu.com/ $(lsb_release -cs)-security main universe restricted multiverse
EOF
apt update
```

### Install dependency libraries

```bash
apt install libssl-dev:armhf libasound2-dev:armhf
```

### Replace standard rust with rustup and activate the target architecture

```bash
apt remove rustc
curl https://sh.rustup.rs -sSf | sh
rustup target add arm-unknown-linux-gnueabihf
```

### Build

```bash
PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig PKG_CONFIG_ALLOW_CROSS=1 cargo build --target=arm-unknown-linux-gnueabihf --release
```

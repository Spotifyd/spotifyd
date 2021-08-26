#!/bin/bash

set -x

ARCH=${1}          # arm, aarch64, i386, x86_64
ABI=${2}           # gnu, gnueabihf
DEBARCH=${3}       # i386, amd64, armhf, arm64
BUILD_TYPE=${4}    # slim, default, full

export DEBIAN_FRONTEND=noninteractive

# Native sources
cat > /etc/apt/sources.list << EOT_NATIVE
deb [arch=amd64] http://archive.ubuntu.com/ubuntu/ focal main restricted universe multiverse
deb [arch=amd64] http://archive.ubuntu.com/ubuntu/ focal-updates main restricted universe multiverse
deb [arch=amd64] http://security.ubuntu.com/ubuntu/ focal-security main restricted universe multiverse
EOT_NATIVE

# If building cross, cross sources
if [ "${ARCH}" != "$(uname -m)" ]; then
	cat > /etc/apt/sources.list.d/cross.list << EOT_CROSS
deb [arch=$DEBARCH] http://ports.ubuntu.com/ focal main restricted universe multiverse
deb [arch=$DEBARCH] http://ports.ubuntu.com/ focal-updates main restricted universe multiverse
deb [arch=$DEBARCH] http://ports.ubuntu.com/ focal-security main restricted universe multiverse
EOT_CROSS
else
	rm -f /etc/apt/sources.list.d/cross.list
fi

dpkg --add-architecture ${DEBARCH}
apt-get update

# Setup magic vars for foreign packages
if [ "${ARCH}" != "$(uname -m)" ]; then
	CROSS=":${DEBARCH}"
	BUILD_PKGS="gcc-${ARCH}-linux-${ABI} libc6-${DEBARCH}-cross libc6-dev-${DEBARCH}-cross pkg-config"
	dpkg --add-architecture ${DEBARCH}
else
	CROSS=""
	BUILD_PKGS="gcc libc6 libc6-dev pkg-config"
fi

# Always install all dependencies, even if we are building default or even slim
BUILDDEP_SLIM="libasound2-dev${CROSS} libssl-dev${CROSS} libpulse-dev${CROSS} libdbus-1-dev${CROSS} libssl1.1${CROSS}"
BUILDDEP_DEFAULT="libdbus-1-dev${CROSS} libdbus-1-3${CROSS} libsystemd0${CROSS} libgcrypt20${CROSS} liblzma5${CROSS} liblz4-1${CROSS} libgpg-error0${CROSS}"
BUILDDEP_FULL="libpulse-dev${CROSS}"

apt-get install -y \
	${BUILD_PKGS} \
	${BUILDDEP_SLIM} \
	${BUILDDEP_DEFAULT} \
	${BUILDDEP_FULL}

# Tell rust to cross-compile
if [ "${ARCH}" != "$(uname -m)" ]; then
	mkdir -p ~/.cargo
	cat >> ~/.cargo/config << EOT_CARGO
[target.${ARCH}-unknown-linux-${ABI}]
linker = "${ARCH}-linux-${ABI}-gcc"

[target.${ARCH}-unknown-linux-${ABI}.dbus]
rustc-link-lib = ["dbus-1", "gcrypt", "gpg-error", "lz4", "lzma", "systemd"]
EOT_CARGO
fi

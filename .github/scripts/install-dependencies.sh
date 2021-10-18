#!/bin/bash

set -e
set -x

ARCH=${1}          # armv6, armhf, arm64
BINARCH=${2}       # arm, aarch64, i386, x86_64
ABI=${3}           # gnu, gnueabihf
DEBARCH=${4}       # i386, amd64, armhf, arm64
BUILD_TYPE=${5}    # slim, default, full

export DEBIAN_FRONTEND=noninteractive

# Native sources
cat | sudo tee /etc/apt/sources.list > /dev/null << EOT_NATIVE
deb [arch=amd64] http://archive.ubuntu.com/ubuntu/ focal main restricted universe multiverse
deb [arch=amd64] http://archive.ubuntu.com/ubuntu/ focal-updates main restricted universe multiverse
deb [arch=amd64] http://security.ubuntu.com/ubuntu/ focal-security main restricted universe multiverse
EOT_NATIVE

# If building cross, cross sources
if [ "${BINARCH}" != "$(uname -m)" ]; then
	cat | sudo tee /etc/apt/sources.list.d/cross.list > /dev/null << EOT_CROSS
deb [arch=$DEBARCH] http://ports.ubuntu.com/ focal main restricted universe multiverse
deb [arch=$DEBARCH] http://ports.ubuntu.com/ focal-updates main restricted universe multiverse
deb [arch=$DEBARCH] http://ports.ubuntu.com/ focal-security main restricted universe multiverse
EOT_CROSS
else
	sudo rm -f /etc/apt/sources.list.d/cross.list
fi

# Setup magic vars for foreign packages
if [ "${BINARCH}" != "$(uname -m)" ]; then
	if [ "${ARCH}" != "armv6" ]; then
		BUILD_PKGS="gcc-${BINARCH}-linux-${ABI} libc6-${DEBARCH}-cross libc6-dev-${DEBARCH}-cross pkg-config"
	else
		BUILD_PKGS="curl libc6-${DEBARCH}-cross libc6-dev-${DEBARCH}-cross pkg-config"
	fi
	CROSS=":${DEBARCH}"
	sudo dpkg --add-architecture ${DEBARCH}
else
	CROSS=""
	BUILD_PKGS="gcc libc6 libc6-dev pkg-config"
fi

sudo apt-get update

# Always install all dependencies, even if we are building default or even slim
BUILDDEP_SLIM="libasound2-dev${CROSS} libssl-dev${CROSS} libpulse-dev${CROSS} libdbus-1-dev${CROSS} libssl1.1${CROSS}"
BUILDDEP_DEFAULT="libdbus-1-dev${CROSS} libdbus-1-3${CROSS} libsystemd0${CROSS} libsystemd-dev${CROSS} libgcrypt20${CROSS} libgcrypt20-dev${CROSS} liblzma5${CROSS} liblzma-dev${CROSS} liblz4-1${CROSS} liblz4-dev${CROSS} libgpg-error0${CROSS} libgpg-error-dev${CROSS}"
BUILDDEP_FULL="libpulse-dev${CROSS}"

sudo apt-get install -y \
	build-essential \
	${BUILD_PKGS} \
	${BUILDDEP_SLIM} \
	${BUILDDEP_DEFAULT} \
	${BUILDDEP_FULL}

if [ "${ARCH}" == "armv6" ]; then
	curl -L -o /tmp/toolchain.tar.bz2 https://toolchains.bootlin.com/downloads/releases/toolchains/armv6-eabihf/tarballs/armv6-eabihf--glibc--stable-2020.08-1.tar.bz2
	sudo mkdir /opt/toolchain
	sudo chown $(whoami) /opt/toolchain
	tar -C /opt/toolchain --strip-components=1 -jxf /tmp/toolchain.tar.bz2
	echo "/opt/toolchain/bin" >> $GITHUB_PATH
fi

# Tell rust to cross-compile
if [ "${BINARCH}" != "$(uname -m)" ]; then
	mkdir -p ~/.cargo
	if [ "${ARCH}" != "armv6" ]; then
		LINKER="${BINARCH}-linux-${ABI}-gcc"
	else
		LINKER="arm-buildroot-linux-gnueabihf-gcc"
	fi
	cat >> ~/.cargo/config << EOT_CARGO
[target.${BINARCH}-unknown-linux-${ABI}]
linker = "${LINKER}"

[target.${BINARCH}-unknown-linux-${ABI}.dbus]
rustc-link-lib = ["dbus-1", "gcrypt", "gpg-error", "lz4", "lzma", "systemd"]
EOT_CARGO
fi

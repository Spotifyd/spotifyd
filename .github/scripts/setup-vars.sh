#!/bin/bash

BUILD_TARGET=${1}  # macos, linux
ARCH=${2}          # armhf, arm64, x86_64
BINARCH=${3}       # arm, aarch64, x86_64
ABI=${4}           # gnu, gnueabihf
ARTIFACT_TYPE=${5} # slim, default, full

# "compute" rust-target
if [ "${BUILD_TARGET}" == "macos" ]; then
	echo RUST_TARGET=x86_64-apple-darwin
else
	echo RUST_TARGET=${BINARCH}-unknown-linux-${ABI}
fi

echo "ARTIFACT_NAME=spotifyd-${BUILD_TARGET}-${ARCH}-${ARTIFACT_TYPE}"

# only on linux
if [ "${BUILD_TARGET}" == "linux" ]; then
	echo "DEBIAN_FRONTEND=noninteractive"
fi

# only when cross-building on linux
if [ "${BUILD_TARGET}" == "linux" ] && [ "${ARCH}" != "$(uname -m)" ]; then
	echo "PKG_CONFIG_ALLOW_CROSS=1"
	echo "PKG_CONFIG_LIBDIR=/usr/lib/${BINARCH}-linux-${ABI}/pkgconfig/"
fi

# raspberry pi toolchain doesn't search in the right places automatically
if [ "${ARCH}" == "armv6" ]; then
	echo "CC=arm-buildroot-linux-gnueabihf-gcc"
	echo "CFLAGS=-march=armv6 -I/usr/include -I/usr/include/arm-linux-gnueabihf -L/usr/lib/arm-linux-gnueabihf" # needed for cc crate in rust-openssl's build main.rs (expando)
	echo "RUSTFLAGS=-Clinker=arm-buildroot-linux-gnueabihf-gcc -L/usr/lib/arm-linux-gnueabihf"
fi

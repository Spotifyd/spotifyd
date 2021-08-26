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
	echo "RUSTFLAGS=-C linker=${BINARCH}-linux-${ABI}-gcc"
	echo "PKG_CONFIG_ALLOW_CROSS=1"
	echo "PKG_CONFIG_LIBDIR=/usr/lib/${BINARCH}-linux-${ABI}/pkgconfig/"
fi

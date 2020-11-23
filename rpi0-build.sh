#!/bin/bash

export PKG_CONFIG_SYSROOT_DIR=/usr/arm-linux-gnueabihf
export PKG_CONFIG_PATH=/usr/arm-linux-gnueabihf/lib

cargo build --release --target arm-unknown-linux-gnueabihf
if [ $1 ]; then
	scp target/arm-unknown-linux-gnueabihf/release/led-strip-controller $1
fi

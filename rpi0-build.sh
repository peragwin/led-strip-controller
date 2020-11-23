#!/bin/bash
export PKG_CONFIG_SYSROOT_DIR=/usr/arm-linux-gnueabihf
export PKG_CONFIG_PATH=/usr/arm-linux-gnueabihf/lib
cargo build --release --target arm-unknown-linux-gnueabihf

#!/bin/bash
set -xeuo pipefail

TARGET="$1"
VERSION="$2"

# Download musl-cross toolchain from cross-tools/musl-cross GitHub releases
cd "$HOME"
curl -Lo "${TARGET}.tar.xz" "https://github.com/cross-tools/musl-cross/releases/download/${VERSION}/${TARGET}.tar.xz"
mkdir -p musl-cross
tar -xf "${TARGET}.tar.xz" -C musl-cross --strip-components=1
rm "${TARGET}.tar.xz"

#!/bin/sh

cronet_example="./cronet_example"

. ./get-sysroot.sh

set -ex

if [ "$WITH_SYSROOT" -a "$WITH_QEMU" ]; then
  cronet_example="qemu-$WITH_QEMU -L $PWD/$WITH_SYSROOT $cronet_example"
fi
if [ "$WITH_ANDROID_IMG" -a "$WITH_QEMU" ]; then
  cronet_example="qemu-$WITH_QEMU -L $PWD/out/sysroot-build/android/$WITH_ANDROID_IMG $cronet_example"
fi

export CC=$PWD/third_party/llvm-build/Release+Asserts/bin/clang
[ "$WITH_GOOS" ] && export GOOS="$WITH_GOOS"
[ "$WITH_GOARCH" ] && export GOARCH="$WITH_GOARCH"
export CGO_ENABLED=1
export CGO_LDFLAGS_ALLOW=.*
cd out/Release/cronet

go build cronet_example.go link_shared.go
$cronet_example
rm -f cronet_example

go build cronet_example.go link_static.go
$cronet_example
rm -f cronet_example

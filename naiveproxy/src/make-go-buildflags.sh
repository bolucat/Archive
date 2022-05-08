#!/bin/sh

. ./get-sysroot.sh

set -ex
if [ "$WITH_SYSROOT" ]; then
  sysroot="--sysroot=$PWD/$WITH_SYSROOT"
fi
cp components/cronet/native/sample/cronet_example.go out/Release/cronet/
shared_ninjafile=./out/Release/obj/components/cronet/cronet_example_external.ninja
shared_ldflags=$(grep '^  ldflags = ' $shared_ninjafile | cut -d= -f2- | sed 's/\$:/:/g;s/\\%/%/g;s/\\\$\$/\$/g' | sed "s#=\.\./\.\./#=$PWD/#g")
shared_libs=$(grep '^  libs = ' $shared_ninjafile | cut -d= -f2)
cat >out/Release/cronet/link_shared.go <<EOF
package main

// #cgo CFLAGS: $sysroot
// #cgo LDFLAGS: $shared_ldflags $sysroot ./libcronet.so $shared_libs
import "C"
EOF

static_ninjafile=./out/Release/obj/components/cronet/cronet_example_external_static.ninja
static_ldflags=$(grep '^  ldflags = ' $static_ninjafile | cut -d= -f2- | sed 's/\$:/:/g;s/\\%/%/g;s/\\\$\$/\$/g' | sed "s#=\.\./\.\./#=$PWD/#g")
static_libs=$(grep '^  libs = ' $static_ninjafile | cut -d= -f2)
cat >out/Release/cronet/link_static.go <<EOF
package main

// #cgo CFLAGS: $sysroot
// #cgo LDFLAGS: $static_ldflags $sysroot ./libcronet_static.a $static_libs -lm
import "C"
EOF

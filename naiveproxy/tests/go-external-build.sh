#!/bin/sh

. ./get-sysroot.sh

set -ex

cd out/Release/cronet

run_cronet_example() {
  if [ "$WITH_QEMU" = "i386" ]; then
    # qemu-i386 doesn't work with CGO compiled i386 executables for some reason.
    cp libcronet.so cronet_example ./sysroot
    sudo LD_LIBRARY_PATH=/ chroot ./sysroot /cronet_example "$@"
    rm ./sysroot/libcronet.so ./sysroot/cronet_example
    return
  fi
  if [ "$WITH_SYSROOT" -a "$WITH_QEMU" ]; then
    qemu-$WITH_QEMU-static -L ./sysroot ./cronet_example "$@"
    return
  fi
  if [ "$WITH_ANDROID_IMG" -a "$WITH_QEMU" ]; then
    qemu-$WITH_QEMU-static -L ./sysroot ./cronet_example "$@"
    return
  fi
  if [ "$target_cpu" = "arm64" -a "$ARCH" = "Darwin" ]; then
    return
  fi
  if [ "$target_cpu" = "arm64" -a "$ARCH" = "Windows" ]; then
    return
  fi
  ./cronet_example "$@"
}

# CGO does not support relative path very well.
for i in go_env.sh link_shared.go link_static.go; do
  sed "s#\./sysroot#$PWD/sysroot#g;
       s#-L\./c/#-L./c/#g;
       s#-L......third_party.llvm-build.Release.Asserts#-L./llvm#g;
       s#libpath:\./c/#libpath:$PWD/c/#g;
       s#libpath:......third_party.llvm-build.Release.Asserts#libpath:$PWD/llvm#g" $i >$i.1
  mv $i.1 $i
done

cat go_env.sh
cat link_shared.go
cat link_static.go

. ./go_env.sh

#strace -o/tmp/trace -f -qq -etrace=execve -s1024 -esignal='!all' \
go build -x $buildmode_flag cronet_example.go link_shared.go
run_cronet_example http://example.com
if [ "$ARCH" = "Linux" ]; then
  ./llvm/llvm-strip cronet_example
fi
if [ "$WITH_CLANG" = "Win" ]; then
  ls -l cronet_example.exe
  rm -f cronet_example.exe
else
  ls -l cronet_example
  rm -f cronet_example
fi

go build -x $buildmode_flag cronet_example.go link_static.go
run_cronet_example http://example.com
if [ "$ARCH" = "Linux" ]; then
  ./llvm/llvm-strip cronet_example
fi
if [ "$WITH_CLANG" = "Win" ]; then
  ls -l cronet_example.exe
  rm -f cronet_example.exe
else
  ls -l cronet_example
  rm -f cronet_example
fi

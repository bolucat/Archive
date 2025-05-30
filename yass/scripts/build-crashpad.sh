#!/bin/bash
set -x
set -e
PWD=$(dirname "${BASH_SOURCE[0]}")
cd $PWD/..

ARCH=$(uname -s)
MACHINE=$(uname -m)
PYTHON=$(which python3 2>/dev/null || which python 2>/dev/null)
CRASHPAD_COMMIT=$(< CRASHPAD_COMMIT)

cd third_party

case "$ARCH" in
  Linux|Darwin)
    if [ ! -d depot_tools ]; then
      git clone --depth 1 https://chromium.googlesource.com/chromium/tools/depot_tools.git
    fi
    export PATH="$PWD/depot_tools:$PATH"
  ;;
  MINGW*|MSYS*)
    ARCH=Windows
    if [ ! -d depot_tools ]; then
      curl -L -O https://storage.googleapis.com/chrome-infra/depot_tools.zip
      curl -L -O https://github.com/ninja-build/ninja/releases/download/v1.11.1/ninja-win.zip
      mkdir depot_tools
      pushd depot_tools
      "/c/Program Files/7-Zip/7z.exe" x ../depot_tools.zip -aoa
      "/c/Program Files/7-Zip/7z.exe" x ../ninja-win.zip -aoa
      rm -f ninja ninja.bat
      # avoid calling to git.bat
      sed -i 's|git.bat|git.exe|g' git_cache.py
      popd
      rm -f depot_tools.zip ninja-win.zip
    fi
    export PATH="$PWD/depot_tools:$PATH"
  ;;
esac

# Ensure that the "depot_tools" has its self-update capability disabled.
"$PYTHON" depot_tools/update_depot_tools_toggle.py --disable

flags="$flags"'
use_sysroot=false'

case "$MACHINE" in
  x86|i586|i686)
    WITH_CPU_DEFAULT="x86"
    ;;
  x86_64)
    WITH_CPU_DEFAULT="x64"
    ;;
  arch64|arm64)
    WITH_CPU_DEFAULT="arm64"
    ;;
esac

WITH_CPU=${WITH_CPU:-${WITH_CPU_DEFAULT}}

if [ "$WITH_CPU" ]; then
  flags="$flags
target_cpu=\"$WITH_CPU\""
fi

case "$ARCH" in
  Darwin)
    WITH_OS_DEFAULT="mac"
    BIN_SUFFIX=""
    ;;
  Linux)
    WITH_OS_DEFAULT="linux"
    BIN_SUFFIX=""
    ;;
  Windows)
    WITH_OS_DEFAULT="win"
    BIN_SUFFIX=".exe"
    ;;
esac

WITH_OS=${WITH_OS:-${WITH_OS_DEFAULT}}
OBJCOPY="$PWD/llvm-build/Release+Asserts/bin/llvm-objcopy"

# strip debug symbols (ignore msvc build which generating pdbs already)
function strip_binary {
if [ ! -z "$SKIP_STRIP" ]; then
  echo 'omit calling llvm-objcopy as required'
  return
fi

local bin_dir="$(dirname $1)"
local bin_name="$(basename $1)"
pushd "$bin_dir"
"$OBJCOPY" --only-keep-debug "${bin_name}" "${bin_name}.dbg"
"$OBJCOPY" --strip-debug "${bin_name}"
"$OBJCOPY" --add-gnu-debuglink="${bin_name}.dbg" "${bin_name}"
popd
}

if [ "$WITH_OS" ]; then
  flags="$flags
target_os=\"$WITH_OS\""
fi

case "$WITH_OS" in
  mac|ios)
    flags="$flags
clang_path=\"$PWD/llvm-build/Release+Asserts\"
extra_cflags_cc=\"-nostdinc++ -I $PWD/libc++ -I $PWD/libc++/trunk/include -D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_EXTENSIVE -D_LIBCPP_DISABLE_VISIBILITY_ANNOTATIONS -D_LIBCPP_OVERRIDABLE_FUNC_VIS='__attribute__((__visibility__(\\\"default\\\")))'\"
extra_cflags_objcc=\"-nostdinc++ -I $PWD/libc++ -I $PWD/libc++/trunk/include -I $PWD/libc++abi/trunk/include -D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_EXTENSIVE -D_LIBCPP_DISABLE_VISIBILITY_ANNOTATIONS -D_LIBCPP_OVERRIDABLE_FUNC_VIS='__attribute__((__visibility__(\\\"default\\\")))'\""
    ;;
  linux|android)
    flags="$flags
clang_path=\"$PWD/llvm-build/Release+Asserts\"
extra_cflags_cc=\"-nostdinc++ -I $PWD/libc++ -I $PWD/libc++/trunk/include -D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_EXTENSIVE -D_LIBCPP_DISABLE_VISIBILITY_ANNOTATIONS -D_LIBCPP_OVERRIDABLE_FUNC_VIS='__attribute__((__visibility__(\\\"default\\\")))'\""
    ;;
  win)
    flags="$flags
clang_path=\"$(cygpath -m $PWD)/llvm-build/Release+Asserts\"
extra_cflags=\"/MD\"
extra_cflags_cc=\"-I $(cygpath -m $PWD)/libc++ -I $(cygpath -m $PWD)/libc++/trunk/include -D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_EXTENSIVE -D_LIBCPP_DISABLE_VISIBILITY_ANNOTATIONS -D_LIBCPP_OVERRIDABLE_FUNC_VIS='__attribute__((__visibility__(\\\"default\\\")))'\""
    ;;
  *)
    echo "Unsupported OS ${WITH_OS}"
    exit 1
    ;;
esac

case "$WITH_OS" in
  mac)
    flags="$flags
mac_deployment_target=\"10.14\""
    ;;
  ios)
    flags="$flags
target_environment=\"device\"
ios_code_signing_identity=\"-\"
ios_deployment_target=\"13.0\""
    ;;
  android)
  os_suffix="-android"
  flags="$flags
android_api_level=24
android_ndk_root=\"$ANDROID_SDK_ROOT/ndk/$ANDROID_NDK_VER\""
    ;;
  *)
    ;;
esac

if [ "$WITH_SYSROOT" ]; then
  flags="$flags
target_sysroot=\"$WITH_SYSROOT\""
fi

bin_flags="$flags
extra_cflags_cc=\"\"
extra_cflags_objcc=\"\""

case "$WITH_OS" in
  ios)
  os_suffix="-ios"
  bin_flags="$bin_flags
extra_cflags_cc=\"-stdlib=libc++\"
extra_cflags_objcc=\"-stdlib=libc++\"
extra_ldflags=\"-stdlib=libc++ -static-libstdc++\""
    ;;
  android)
  os_suffix="-android"
  bin_flags="$bin_flags
extra_cflags_cc=\"-stdlib=libc++\"
extra_ldflags=\"-stdlib=libc++ -static-libstdc++\""
    ;;
  *)
    ;;
esac

out="$PWD/crashpad/crashpad/out/Default-${WITH_CPU}${os_suffix}"
bin_out="$PWD/crashpad/crashpad/out/Binary-${WITH_CPU}${os_suffix}"

export DEPOT_TOOLS_WIN_TOOLCHAIN=0

mkdir -p crashpad
cd crashpad
fetch --nohistory crashpad || true
cd crashpad
git fetch origin $CRASHPAD_COMMIT
git checkout -f $CRASHPAD_COMMIT
git reset --hard
gclient sync -f

# patch stage
case "$ARCH" in
  Darwin)
    sed="sed -i '' -e"
    ;;
  *)
    sed="sed -i"
    ;;
esac

cp -f ../../../scripts/mini_chromium.BUILD.gn third_party/mini_chromium/mini_chromium/build/config/BUILD.gn
$sed 's|__hlt(0)|asm volatile("hlt #0")|g' third_party/mini_chromium/mini_chromium/base/immediate_crash.h
patch -p1 < ../../../scripts/crashpad_mips.patch

# build stage
rm -rf "$out"
mkdir -p "$out"
echo "$flags" > "$out/args.gn"
gn gen "$out" --script-executable="$PYTHON" --export-compile-comman
ninja -C "$out" client

case "$WITH_OS" in
  ios)
    exit 0
    ;;
  **)
    ;;
esac

rm -rf "$bin_out"
mkdir -p "$bin_out"
echo "$bin_flags" > "$bin_out/args.gn"
gn gen "$bin_out" --script-executable="$PYTHON" --export-compile-comman
ninja -C "$bin_out" crashpad_handler
strip_binary "${bin_out}/crashpad_handler${BIN_SUFFIX}"

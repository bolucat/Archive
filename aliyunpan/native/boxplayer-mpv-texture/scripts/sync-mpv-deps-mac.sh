#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PACKAGE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCE_PREFIX="${LIBMPV_PREFIX:-/opt/homebrew}"
SOURCE_LIBRARY="${LIBMPV_LIBRARY:-$SOURCE_PREFIX/lib/libmpv.dylib}"
OUTPUT_ROOT="${MPV_DEPS_DIR:-$PACKAGE_DIR/deps/mpv}"
OUTPUT_INCLUDE="$OUTPUT_ROOT/include/mpv"
OUTPUT_MACOS="$OUTPUT_ROOT/macos"

for header in client.h render.h render_gl.h; do
  if [[ ! -f "$SOURCE_PREFIX/include/mpv/$header" ]]; then
    echo "missing: $SOURCE_PREFIX/include/mpv/$header" >&2
    exit 1
  fi
done

if [[ ! -f "$SOURCE_LIBRARY" ]]; then
  echo "missing: $SOURCE_LIBRARY" >&2
  exit 1
fi

mkdir -p "$OUTPUT_INCLUDE" "$OUTPUT_MACOS"
cp "$SOURCE_PREFIX/include/mpv/"*.h "$OUTPUT_INCLUDE/"
cp "$SOURCE_LIBRARY" "$OUTPUT_MACOS/libmpv.dylib"
chmod 755 "$OUTPUT_MACOS/libmpv.dylib"
install_name_tool -id "@rpath/libmpv.dylib" "$OUTPUT_MACOS/libmpv.dylib" 2>/dev/null || true
codesign --force --sign - "$OUTPUT_MACOS/libmpv.dylib" 2>/dev/null || true

echo "synced libmpv SDK"
echo "source: $SOURCE_PREFIX"
echo "headers: $OUTPUT_INCLUDE"
echo "library: $OUTPUT_MACOS/libmpv.dylib"

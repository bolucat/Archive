#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PACKAGE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_PREFIX="$PACKAGE_DIR/deps/mpv"

prefix="${LIBMPV_PREFIX:-}"
if [[ -z "$prefix" ]]; then
  for candidate in "$PROJECT_PREFIX" /opt/homebrew /usr/local; do
    if [[ -f "$candidate/include/mpv/client.h" || -f "$candidate/include/mpv/render.h" ]]; then
      prefix="$candidate"
      break
    fi
  done
fi

if [[ -z "$prefix" ]]; then
  echo "libmpv headers not found. Set LIBMPV_PREFIX to an SDK prefix containing include/mpv/client.h and include/mpv/render.h." >&2
  exit 1
fi

missing=0
for header in client.h render.h; do
  if [[ ! -f "$prefix/include/mpv/$header" ]]; then
    echo "missing: $prefix/include/mpv/$header" >&2
    missing=1
  fi
done

library="${LIBMPV_LIBRARY:-$prefix/lib/libmpv.dylib}"
if [[ ! -f "$library" && -f "$prefix/macos/libmpv.dylib" ]]; then
  library="$prefix/macos/libmpv.dylib"
fi
if [[ ! -f "$library" ]]; then
  echo "missing: $library" >&2
  missing=1
fi

if [[ "$missing" -ne 0 ]]; then
  exit 1
fi

echo "libmpv SDK ready: $prefix"
echo "headers: $prefix/include/mpv"
echo "library: $library"

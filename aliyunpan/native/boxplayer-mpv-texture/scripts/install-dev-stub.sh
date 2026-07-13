#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PACKAGE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd "$PACKAGE_DIR/../.." && pwd)"
ARCH="${ARCH:-$(uname -m)}"

if [[ "$ARCH" == "aarch64" ]]; then
  ARCH="arm64"
fi

NODE_FILE="$PACKAGE_DIR/build/Release/boxplayer-mpv-texture.node"
OUTPUT_DIR="$REPO_DIR/static/engine/darwin/$ARCH/mpv-texture"

if [[ ! -f "$NODE_FILE" ]]; then
  echo "[install-dev-stub] missing $NODE_FILE" >&2
  echo "[install-dev-stub] build the default stub addon first." >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
cp "$NODE_FILE" "$OUTPUT_DIR/boxplayer-mpv-texture.node"

echo "[install-dev-stub] copied stub addon to $OUTPUT_DIR/boxplayer-mpv-texture.node"

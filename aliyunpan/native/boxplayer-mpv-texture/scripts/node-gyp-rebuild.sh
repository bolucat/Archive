#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PACKAGE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(cd "$PACKAGE_DIR/../.." && pwd)"
NODE_GYP_BIN="${NODE_GYP_BIN:-$REPO_DIR/node_modules/.pnpm/node-gyp@12.4.0/node_modules/node-gyp/bin/node-gyp.js}"
NODE_DIR="${NODE_DIR:-$HOME/.hermes/node}"

if [[ ! -f "$NODE_GYP_BIN" ]]; then
  echo "missing node-gyp: $NODE_GYP_BIN" >&2
  exit 1
fi

if [[ ! -d "$NODE_DIR/include/node" ]]; then
  echo "missing Node headers: $NODE_DIR/include/node" >&2
  exit 1
fi

cd "$PACKAGE_DIR"
node "$NODE_GYP_BIN" rebuild --nodedir="$NODE_DIR" "$@"

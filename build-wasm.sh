#!/usr/bin/env bash
# Build the WASM demo. Requires a modern Rust toolchain (this port's
# core targets rustc 1.75, but wasm-bindgen's macro crate needs 1.77+)
# and wasm-pack.
#
#   rustup update
#   cargo install wasm-pack   # if not already installed
#   bash build-wasm.sh
#
# Output goes to web/pkg/, which web/demo.html loads directly -- no
# bundler needed.

set -euo pipefail
cd "$(dirname "$0")/web/wasm"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "wasm-pack not found. Install it with:" >&2
  echo "  cargo install wasm-pack" >&2
  echo "or see https://rustwasm.github.io/wasm-pack/installer/" >&2
  exit 1
fi

wasm-pack build --target web --out-dir ../pkg --release

echo
echo "Built. Open web/demo.html directly in a browser (no server needed)"
echo "or serve web/ with any static file server."

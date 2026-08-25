#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WASM_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$WASM_DIR/../.." && pwd)"
OUT_DIR="${1:-$WASM_DIR/build/web}"
WASM_BINDGEN_VERSION="0.2.127"

command -v cargo >/dev/null 2>&1 || {
  echo "cargo is required" >&2
  exit 1
}
command -v rustup >/dev/null 2>&1 || {
  echo "rustup is required" >&2
  exit 1
}
command -v wasm-bindgen >/dev/null 2>&1 || {
  echo "wasm-bindgen CLI ${WASM_BINDGEN_VERSION} is required" >&2
  exit 1
}

ACTUAL_BINDGEN_VERSION="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$ACTUAL_BINDGEN_VERSION" != "$WASM_BINDGEN_VERSION" ]; then
  echo "wasm-bindgen CLI must be ${WASM_BINDGEN_VERSION}; found ${ACTUAL_BINDGEN_VERSION}" >&2
  exit 1
fi

rustup target add wasm32-unknown-unknown
cargo build \
  --manifest-path "$WASM_DIR/Cargo.toml" \
  --target wasm32-unknown-unknown \
  --release

RAW_WASM="$WASM_DIR/target/wasm32-unknown-unknown/release/fresnica_wasm_sdk.wasm"
test -s "$RAW_WASM"

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"
wasm-bindgen \
  --target web \
  --typescript \
  --out-name fresnica_sdk \
  --out-dir "$OUT_DIR" \
  "$RAW_WASM"

cat > "$OUT_DIR/package.json" <<'JSON'
{
  "name": "@fresnica/wasm-sdk",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "sideEffects": false,
  "files": [
    "fresnica_sdk.js",
    "fresnica_sdk_bg.wasm",
    "fresnica_sdk.d.ts"
  ],
  "module": "./fresnica_sdk.js",
  "types": "./fresnica_sdk.d.ts"
}
JSON

printf 'Fresnica Web/WASM SDK ready at %s\n' "$OUT_DIR"

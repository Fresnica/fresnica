#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WASM_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$WASM_DIR/../.." && pwd)"

command -v node >/dev/null 2>&1 || {
  echo "node is required" >&2
  exit 1
}
command -v cargo >/dev/null 2>&1 || {
  echo "cargo is required" >&2
  exit 1
}
command -v rustup >/dev/null 2>&1 || {
  echo "rustup is required" >&2
  exit 1
}

node "$WASM_DIR/tests/boundary.mjs"
cargo fmt --manifest-path "$REPO_ROOT/sdk/rust/Cargo.toml" -- --check
cargo fmt --manifest-path "$WASM_DIR/Cargo.toml" -- --check
cargo test --manifest-path "$REPO_ROOT/sdk/rust/Cargo.toml"
rustup target add wasm32-unknown-unknown
cargo check --manifest-path "$WASM_DIR/Cargo.toml" --target wasm32-unknown-unknown
bash "$WASM_DIR/scripts/build-web.sh"
node "$WASM_DIR/tests/generated-surface.mjs" "$WASM_DIR/build/web"
node "$WASM_DIR/tests/runtime-conformance.mjs" "$WASM_DIR/build/web"

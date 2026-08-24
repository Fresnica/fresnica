#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
CRATE_DIR="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="${1:-$CRATE_DIR/build/apple}"
DEPLOYMENT_TARGET="${FRESNICA_IOS_DEPLOYMENT_TARGET:-13.4}"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "Apple packaging must run on macOS" >&2
  exit 1
fi

for tool in cargo rustup lipo xcodebuild; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is required" >&2
    exit 1
  fi
done

rustup target add \
  aarch64-apple-ios \
  aarch64-apple-ios-sim \
  x86_64-apple-ios

rm -rf "$OUTPUT_DIR"
mkdir -p \
  "$OUTPUT_DIR/device" \
  "$OUTPUT_DIR/simulator" \
  "$OUTPUT_DIR/generated-swift" \
  "$OUTPUT_DIR/headers"

cd "$CRATE_DIR"
export IPHONEOS_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET"

cargo build --release --target aarch64-apple-ios
cargo build --release --target aarch64-apple-ios-sim
cargo build --release --target x86_64-apple-ios

cp target/aarch64-apple-ios/release/libfresnica_mobile_core.a \
  "$OUTPUT_DIR/device/libfresnica_mobile_core.a"

lipo -create \
  target/aarch64-apple-ios-sim/release/libfresnica_mobile_core.a \
  target/x86_64-apple-ios/release/libfresnica_mobile_core.a \
  -output "$OUTPUT_DIR/simulator/libfresnica_mobile_core.a"

# Build a host library only so uniffi-bindgen can read the embedded metadata.
cargo build --release
cargo run --features bindgen --bin uniffi-bindgen -- \
  generate --library target/release/libfresnica_mobile_core.dylib \
  --language swift --out-dir "$OUTPUT_DIR/generated-swift"

SWIFT_SOURCE="$OUTPUT_DIR/generated-swift/FresnicaCore.swift"
FFI_HEADER="$OUTPUT_DIR/generated-swift/FresnicaCoreFFI.h"
FFI_MODULEMAP="$OUTPUT_DIR/generated-swift/FresnicaCoreFFI.modulemap"

for generated in "$SWIFT_SOURCE" "$FFI_HEADER" "$FFI_MODULEMAP"; do
  if [ ! -s "$generated" ]; then
    echo "missing generated Swift binding artifact: $generated" >&2
    exit 1
  fi
done

cp "$FFI_HEADER" "$OUTPUT_DIR/headers/FresnicaCoreFFI.h"
cp "$FFI_MODULEMAP" "$OUTPUT_DIR/headers/module.modulemap"

xcodebuild -create-xcframework \
  -library "$OUTPUT_DIR/device/libfresnica_mobile_core.a" \
  -headers "$OUTPUT_DIR/headers" \
  -library "$OUTPUT_DIR/simulator/libfresnica_mobile_core.a" \
  -headers "$OUTPUT_DIR/headers" \
  -output "$OUTPUT_DIR/FresnicaCoreFFI.xcframework"

test -d "$OUTPUT_DIR/FresnicaCoreFFI.xcframework"

printf 'Apple native package ready at %s\n' "$OUTPUT_DIR"

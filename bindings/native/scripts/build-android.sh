#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
CRATE_DIR="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
JNI_DIR="${1:-$CRATE_DIR/build/android/jniLibs}"
PACKAGE_DIR="$(dirname "$JNI_DIR")"
KOTLIN_DIR="${FRESNICA_KOTLIN_OUTPUT:-$PACKAGE_DIR/kotlin}"
ANDROID_API="${FRESNICA_ANDROID_API:-26}"

ABIS=(armeabi-v7a x86 x86_64 arm64-v8a)
RUST_TARGETS=(
  armv7-linux-androideabi
  i686-linux-android
  x86_64-linux-android
  aarch64-linux-android
)

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required" >&2
  exit 1
fi

if ! cargo ndk --version >/dev/null 2>&1; then
  echo "cargo-ndk is required" >&2
  exit 1
fi

if [ -z "${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}" ]; then
  echo "ANDROID_NDK_HOME or ANDROID_NDK_ROOT must point to the Android NDK" >&2
  exit 1
fi

rustup target add "${RUST_TARGETS[@]}"

rm -rf "$JNI_DIR" "$KOTLIN_DIR"
mkdir -p "$JNI_DIR" "$KOTLIN_DIR"

cd "$CRATE_DIR"
cargo ndk \
  -P "$ANDROID_API" \
  -t armeabi-v7a \
  -t x86 \
  -t x86_64 \
  -t arm64-v8a \
  -o "$JNI_DIR" \
  build --release

for abi in "${ABIS[@]}"; do
  library="$JNI_DIR/$abi/libfresnica_native_sdk.so"
  if [ ! -s "$library" ]; then
    echo "missing Android library: $library" >&2
    exit 1
  fi

  extra="$(find "$JNI_DIR/$abi" -maxdepth 1 -type f -name '*.so' ! -name 'libfresnica_native_sdk.so' -print)"
  if [ -n "$extra" ]; then
    echo "unexpected Android shared libraries for $abi:" >&2
    echo "$extra" >&2
    exit 1
  fi
done

# Generate the stable Kotlin native API from host metadata. Framework-specific
# adapter source is deliberately not part of this package.
cargo build --release
HOST_LIBRARY="target/release/libfresnica_native_sdk.so"
if [ "$(uname -s)" = "Darwin" ]; then
  HOST_LIBRARY="target/release/libfresnica_native_sdk.dylib"
fi

cargo run --features bindgen --bin uniffi-bindgen -- \
  generate --library "$HOST_LIBRARY" \
  --language kotlin --out-dir "$KOTLIN_DIR"

if ! grep -R -q "package com.fresnica.sdk" "$KOTLIN_DIR"; then
  echo "generated Kotlin package is missing com.fresnica.sdk" >&2
  exit 1
fi
if ! grep -R -q "FresnicaSdkApi" "$KOTLIN_DIR"; then
  echo "generated Kotlin API is missing FresnicaSdkApi" >&2
  exit 1
fi
if grep -R -q -E 'ReactNative|ReactContext|TurboModule|io\.flutter' "$KOTLIN_DIR"; then
  echo "framework-specific adapter code leaked into Native SDK Kotlin output" >&2
  exit 1
fi

printf 'Android Native SDK package ready at %s\n' "$PACKAGE_DIR"

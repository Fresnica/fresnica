#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
CRATE_DIR="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="${1:-$CRATE_DIR/build/android/jniLibs}"
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

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

cd "$CRATE_DIR"
cargo ndk \
  -P "$ANDROID_API" \
  -t armeabi-v7a \
  -t x86 \
  -t x86_64 \
  -t arm64-v8a \
  -o "$OUTPUT_DIR" \
  build --release

for abi in "${ABIS[@]}"; do
  library="$OUTPUT_DIR/$abi/libfresnica_mobile_core.so"
  if [ ! -s "$library" ]; then
    echo "missing Android library: $library" >&2
    exit 1
  fi

  extra="$(find "$OUTPUT_DIR/$abi" -maxdepth 1 -type f -name '*.so' ! -name 'libfresnica_mobile_core.so' -print)"
  if [ -n "$extra" ]; then
    echo "unexpected Android shared libraries for $abi:" >&2
    echo "$extra" >&2
    exit 1
  fi
done

printf 'Android native package ready at %s\n' "$OUTPUT_DIR"

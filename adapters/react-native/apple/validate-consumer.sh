#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ADAPTER_DIR="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
REPO_DIR="$(CDPATH= cd -- "$ADAPTER_DIR/../.." && pwd)"
PROJECT_DIR="${1:?consumer React Native project path is required}"
NATIVE_BUILD_DIR="${2:-$REPO_DIR/bindings/native/build/apple}"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "Apple React Native consumer validation must run on macOS" >&2
  exit 1
fi

for tool in node xcrun lipo; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is required" >&2
    exit 1
  fi
done

if [ ! -s "$PROJECT_DIR/package.json" ]; then
  echo "consumer package.json is required: $PROJECT_DIR/package.json" >&2
  exit 1
fi
if [ ! -d "$PROJECT_DIR/ios/Pods" ]; then
  echo "React Native CocoaPods installation is required; run pod install in the consumer iOS project first" >&2
  exit 1
fi

SDK_XCFRAMEWORK="$NATIVE_BUILD_DIR/FresnicaSDK.xcframework"
FFI_XCFRAMEWORK="$NATIVE_BUILD_DIR/FresnicaSDKFFI.xcframework"
if [ ! -d "$SDK_XCFRAMEWORK" ] || [ ! -d "$FFI_XCFRAMEWORK" ]; then
  bash "$REPO_DIR/bindings/native/scripts/validate-apple-local.sh" "$NATIVE_BUILD_DIR"
fi

for path in "$SDK_XCFRAMEWORK" "$FFI_XCFRAMEWORK"; do
  if [ ! -d "$path" ]; then
    echo "missing validated Native SDK XCFramework: $path" >&2
    exit 1
  fi
done

BUILD_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fresnica-rn-consumer.XXXXXX")"
trap 'rm -rf "$BUILD_DIR"' EXIT

node "$ADAPTER_DIR/tooling/fresnica-adapter.mjs" \
  build react-native \
  --platform apple \
  --project "$PROJECT_DIR" \
  --native-apple-sdk-xcframework "$SDK_XCFRAMEWORK" \
  --native-apple-ffi-xcframework "$FFI_XCFRAMEWORK" \
  --out "$BUILD_DIR/output"

XCFRAMEWORK="$BUILD_DIR/output/FresnicaRNAdapter.xcframework"
MANIFEST="$BUILD_DIR/output/adapter-manifest.json"
test -d "$XCFRAMEWORK"
test -s "$MANIFEST"

node "$ADAPTER_DIR/tooling/adapter-manifest.mjs" check \
  --project "$PROJECT_DIR" \
  --manifest "$MANIFEST"

DEVICE_LIBRARY="$(find "$XCFRAMEWORK" -type f -name libFresnicaRNAdapter.a ! -path '*simulator*' -print -quit)"
SIMULATOR_LIBRARY="$(find "$XCFRAMEWORK" -type f -name libFresnicaRNAdapter.a -path '*simulator*' -print -quit)"
for library in "$DEVICE_LIBRARY" "$SIMULATOR_LIBRARY"; do
  if [ -z "$library" ] || [ ! -s "$library" ]; then
    echo "missing React Native adapter Apple slice" >&2
    exit 1
  fi
done

DEVICE_INFO="$(xcrun lipo -info "$DEVICE_LIBRARY")"
SIMULATOR_INFO="$(xcrun lipo -info "$SIMULATOR_LIBRARY")"
printf '%s\n' "$DEVICE_INFO" | grep -q 'arm64'
printf '%s\n' "$SIMULATOR_INFO" | grep -q 'arm64'
printf '%s\n' "$SIMULATOR_INFO" | grep -q 'x86_64'

printf 'Fresnica React Native Apple consumer validation: OK\n'
printf '  React Native project: %s\n' "$PROJECT_DIR"
printf '  Native SDK: %s\n' "$SDK_XCFRAMEWORK"
printf '  Adapter slices: device arm64; simulator arm64+x86_64\n'

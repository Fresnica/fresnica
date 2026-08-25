#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
PROJECT_DIR="${1:?consumer React Native project path is required}"
NATIVE_SDK_XCFRAMEWORK="${2:?FresnicaSDK.xcframework path is required}"
NATIVE_FFI_XCFRAMEWORK="${3:?FresnicaSDKFFI.xcframework path is required}"
OUTPUT_XCFRAMEWORK="${4:?output XCFramework path is required}"
DEPLOYMENT_TARGET="13.4"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "Apple React Native adapter packaging must run on macOS" >&2
  exit 1
fi

for tool in xcrun xcodebuild; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is required" >&2
    exit 1
  fi
done

for path in "$NATIVE_SDK_XCFRAMEWORK" "$NATIVE_FFI_XCFRAMEWORK"; do
  if [ ! -d "$path" ]; then
    echo "missing Native SDK XCFramework: $path" >&2
    exit 1
  fi
done

PODS_DIR="$PROJECT_DIR/ios/Pods"
PODS_PUBLIC="$PODS_DIR/Headers/Public"
PODS_PRIVATE="$PODS_DIR/Headers/Private"
if [ ! -d "$PODS_DIR" ]; then
  echo "React Native CocoaPods installation is required; run pod install in the consumer iOS project first" >&2
  exit 1
fi

SWIFT_SOURCE="$SCRIPT_DIR/FresnicaCoreModule.swift"
OBJC_SOURCE="$SCRIPT_DIR/FresnicaCoreModule.m"
for source in "$SWIFT_SOURCE" "$OBJC_SOURCE"; do
  if [ ! -s "$source" ]; then
    echo "missing canonical Apple adapter source: $source" >&2
    exit 1
  fi
done

find_sdk_framework() {
  mode="$1"
  if [ "$mode" = "simulator" ]; then
    find "$NATIVE_SDK_XCFRAMEWORK" -type d -name FresnicaSDK.framework -path '*simulator*' -print -quit
  else
    find "$NATIVE_SDK_XCFRAMEWORK" -type d -name FresnicaSDK.framework \
      ! -path '*simulator*' ! -path '*macos*' -print -quit
  fi
}

find_ffi_headers() {
  mode="$1"
  if [ "$mode" = "simulator" ]; then
    find "$NATIVE_FFI_XCFRAMEWORK" -type d -name Headers -path '*simulator*' -print -quit
  else
    find "$NATIVE_FFI_XCFRAMEWORK" -type d -name Headers \
      ! -path '*simulator*' ! -path '*macos*' -print -quit
  fi
}

DEVICE_FRAMEWORK="$(find_sdk_framework device)"
SIMULATOR_FRAMEWORK="$(find_sdk_framework simulator)"
DEVICE_FFI_HEADERS="$(find_ffi_headers device)"
SIMULATOR_FFI_HEADERS="$(find_ffi_headers simulator)"

for path in "$DEVICE_FRAMEWORK" "$SIMULATOR_FRAMEWORK" "$DEVICE_FFI_HEADERS" "$SIMULATOR_FFI_HEADERS"; do
  if [ -z "$path" ] || [ ! -e "$path" ]; then
    echo "unable to locate a required Native SDK Apple slice" >&2
    exit 1
  fi
done

BUILD_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fresnica-rn-apple.XXXXXX")"
trap 'rm -rf "$BUILD_DIR"' EXIT
mkdir -p "$BUILD_DIR/device" "$BUILD_DIR/sim-arm64" "$BUILD_DIR/sim-x86_64" "$BUILD_DIR/simulator"

react_header_flags=()
for headers_dir in "$PODS_PUBLIC" "$PODS_PRIVATE"; do
  if [ -d "$headers_dir" ]; then
    while IFS= read -r header_root; do
      react_header_flags+=("-I" "$header_root")
    done < <(find "$headers_dir" -mindepth 0 -maxdepth 1 -type d -print | sort)
  fi
done

find_react_native_root() {
  search_dir="$PROJECT_DIR"
  while :; do
    candidate="$search_dir/node_modules/react-native"
    if [ -s "$candidate/React/Base/RCTBridgeModule.h" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi

    parent_dir="$(dirname "$search_dir")"
    if [ "$parent_dir" = "$search_dir" ]; then
      break
    fi
    search_dir="$parent_dir"
  done
  return 1
}

REACT_BRIDGE_HEADER="$(find "$PODS_DIR" \
  \( -type f -o -type l \) \
  -path '*/React/RCTBridgeModule.h' \
  -print -quit)"

if [ -n "$REACT_BRIDGE_HEADER" ]; then
  REACT_BRIDGE_ROOT="$(dirname "$(dirname "$REACT_BRIDGE_HEADER")")"
  react_header_flags+=("-I" "$REACT_BRIDGE_ROOT")
else
  REACT_NATIVE_ROOT="$(find_react_native_root || true)"
  if [ -z "$REACT_NATIVE_ROOT" ]; then
    echo "unable to locate React/RCTBridgeModule.h under $PODS_DIR or react-native under node_modules" >&2
    exit 1
  fi
  if ! command -v ruby >/dev/null 2>&1; then
    echo "ruby is required to reconstruct CocoaPods React Native source header namespaces" >&2
    exit 1
  fi

  POD_HEADER_SHIM="$SCRIPT_DIR/pod-header-shim.rb"
  if [ ! -s "$POD_HEADER_SHIM" ]; then
    echo "missing CocoaPods header resolver: $POD_HEADER_SHIM" >&2
    exit 1
  fi

  while IFS= read -r header_root; do
    [ -n "$header_root" ] && react_header_flags+=("-I" "$header_root")
  done < <(ruby "$POD_HEADER_SHIM" \
    "$REACT_NATIVE_ROOT" \
    "$PODS_DIR" \
    "$BUILD_DIR/react-source-headers")
fi

compile_slice() {
  sdk_name="$1"
  target="$2"
  framework="$3"
  ffi_headers="$4"
  slice_dir="$5"

  sdk_root="$(xcrun --sdk "$sdk_name" --show-sdk-path)"
  xcrun --sdk "$sdk_name" swiftc \
    -target "$target" \
    -sdk "$sdk_root" \
    -parse-as-library \
    -O \
    -module-name FresnicaReactNativeAdapter \
    -F "$(dirname "$framework")" \
    -I "$ffi_headers" \
    -c "$SWIFT_SOURCE" \
    -o "$slice_dir/FresnicaCoreModule.o"

  xcrun --sdk "$sdk_name" clang \
    -target "$target" \
    -isysroot "$sdk_root" \
    -fobjc-arc \
    -fmodules \
    "${react_header_flags[@]}" \
    -c "$OBJC_SOURCE" \
    -o "$slice_dir/FresnicaCoreModuleBridge.o"

  xcrun libtool -static \
    -o "$slice_dir/libFresnicaRNAdapter.a" \
    "$slice_dir/FresnicaCoreModule.o" \
    "$slice_dir/FresnicaCoreModuleBridge.o"
}

compile_slice \
  iphoneos \
  "arm64-apple-ios${DEPLOYMENT_TARGET}" \
  "$DEVICE_FRAMEWORK" \
  "$DEVICE_FFI_HEADERS" \
  "$BUILD_DIR/device"

compile_slice \
  iphonesimulator \
  "arm64-apple-ios${DEPLOYMENT_TARGET}-simulator" \
  "$SIMULATOR_FRAMEWORK" \
  "$SIMULATOR_FFI_HEADERS" \
  "$BUILD_DIR/sim-arm64"

compile_slice \
  iphonesimulator \
  "x86_64-apple-ios${DEPLOYMENT_TARGET}-simulator" \
  "$SIMULATOR_FRAMEWORK" \
  "$SIMULATOR_FFI_HEADERS" \
  "$BUILD_DIR/sim-x86_64"

xcrun lipo -create \
  "$BUILD_DIR/sim-arm64/libFresnicaRNAdapter.a" \
  "$BUILD_DIR/sim-x86_64/libFresnicaRNAdapter.a" \
  -output "$BUILD_DIR/simulator/libFresnicaRNAdapter.a"

rm -rf "$OUTPUT_XCFRAMEWORK"
mkdir -p "$(dirname "$OUTPUT_XCFRAMEWORK")"
xcodebuild -create-xcframework \
  -library "$BUILD_DIR/device/libFresnicaRNAdapter.a" \
  -library "$BUILD_DIR/simulator/libFresnicaRNAdapter.a" \
  -output "$OUTPUT_XCFRAMEWORK"

test -d "$OUTPUT_XCFRAMEWORK"
printf 'Apple React Native adapter ready at %s\n' "$OUTPUT_XCFRAMEWORK"

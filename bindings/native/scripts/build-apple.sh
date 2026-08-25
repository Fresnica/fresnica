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

for tool in cargo rustup xcrun lipo xcodebuild; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is required" >&2
    exit 1
  fi
done

# CommandLineTools only ships the macOS SDK. Prefer a full Xcode installation
# for this process without changing the user's global xcode-select setting.
if ! xcrun --sdk iphoneos --show-sdk-path >/dev/null 2>&1; then
  if [ -d /Applications/Xcode.app/Contents/Developer ]; then
    export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
  fi
fi

if ! IOS_SDK="$(xcrun --sdk iphoneos --show-sdk-path 2>/dev/null)"; then
  echo "full Xcode with the iPhoneOS SDK is required" >&2
  echo "set DEVELOPER_DIR=/path/to/Xcode.app/Contents/Developer and retry" >&2
  exit 1
fi
if ! SIMULATOR_SDK="$(xcrun --sdk iphonesimulator --show-sdk-path 2>/dev/null)"; then
  echo "full Xcode with the iPhoneSimulator SDK is required" >&2
  exit 1
fi

rustup target add \
  aarch64-apple-ios \
  aarch64-apple-ios-sim \
  x86_64-apple-ios

rm -rf "$OUTPUT_DIR"
mkdir -p \
  "$OUTPUT_DIR/device" \
  "$OUTPUT_DIR/simulator" \
  "$OUTPUT_DIR/generated-swift" \
  "$OUTPUT_DIR/headers" \
  "$OUTPUT_DIR/platform-security"

cd "$CRATE_DIR"
export IPHONEOS_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET"

SDKROOT="$IOS_SDK" cargo build --release --target aarch64-apple-ios
SDKROOT="$SIMULATOR_SDK" cargo build --release --target aarch64-apple-ios-sim
SDKROOT="$SIMULATOR_SDK" cargo build --release --target x86_64-apple-ios

cp target/aarch64-apple-ios/release/libfresnica_native_sdk.a \
  "$OUTPUT_DIR/device/libfresnica_native_sdk.a"

lipo -create \
  target/aarch64-apple-ios-sim/release/libfresnica_native_sdk.a \
  target/x86_64-apple-ios/release/libfresnica_native_sdk.a \
  -output "$OUTPUT_DIR/simulator/libfresnica_native_sdk.a"

# Build a host library only so uniffi-bindgen can read embedded metadata.
cargo build --release
cargo run --features bindgen --bin uniffi-bindgen -- \
  generate --library target/release/libfresnica_native_sdk.dylib \
  --language swift --out-dir "$OUTPUT_DIR/generated-swift"

SWIFT_SOURCE="$OUTPUT_DIR/generated-swift/FresnicaSDK.swift"
FFI_HEADER="$OUTPUT_DIR/generated-swift/FresnicaSDKFFI.h"
FFI_MODULEMAP="$OUTPUT_DIR/generated-swift/FresnicaSDKFFI.modulemap"

for generated in "$SWIFT_SOURCE" "$FFI_HEADER" "$FFI_MODULEMAP"; do
  if [ ! -s "$generated" ]; then
    echo "missing generated Swift binding artifact: $generated" >&2
    exit 1
  fi
done

if grep -q -E 'ReactNative|RCTBridge|Flutter' "$SWIFT_SOURCE"; then
  echo "framework-specific adapter code leaked into Native SDK Swift output" >&2
  exit 1
fi

cp "$FFI_HEADER" "$OUTPUT_DIR/headers/FresnicaSDKFFI.h"
cp "$FFI_MODULEMAP" "$OUTPUT_DIR/headers/module.modulemap"
cp "$CRATE_DIR/platform/apple/FresnicaWalletUnlockKeyStore.swift" "$OUTPUT_DIR/platform-security/"
cp "$CRATE_DIR/platform/apple/FresnicaSignerAuthorization.swift" "$OUTPUT_DIR/platform-security/"

xcodebuild -create-xcframework \
  -library "$OUTPUT_DIR/device/libfresnica_native_sdk.a" \
  -headers "$OUTPUT_DIR/headers" \
  -library "$OUTPUT_DIR/simulator/libfresnica_native_sdk.a" \
  -headers "$OUTPUT_DIR/headers" \
  -output "$OUTPUT_DIR/FresnicaSDKFFI.xcframework"

test -d "$OUTPUT_DIR/FresnicaSDKFFI.xcframework"

# Build an importable Swift framework above the low-level FFI XCFramework. The temporary
# Swift package is a packaging mechanism only; consumers receive compiled XCFrameworks and
# never run Rust or UniFFI generation.
SWIFT_PACKAGE_DIR="$OUTPUT_DIR/swift-package"
mkdir -p "$SWIFT_PACKAGE_DIR/Sources/FresnicaSDK"
cp "$SWIFT_SOURCE" "$SWIFT_PACKAGE_DIR/Sources/FresnicaSDK/FresnicaSDK.swift"
cp "$OUTPUT_DIR/platform-security/FresnicaWalletUnlockKeyStore.swift" \
  "$SWIFT_PACKAGE_DIR/Sources/FresnicaSDK/"
cp "$OUTPUT_DIR/platform-security/FresnicaSignerAuthorization.swift" \
  "$SWIFT_PACKAGE_DIR/Sources/FresnicaSDK/"
cat > "$SWIFT_PACKAGE_DIR/Package.swift" <<'SWIFT_PACKAGE'
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "FresnicaSDK",
    platforms: [.iOS(.v13)],
    products: [
        .library(name: "FresnicaSDK", type: .dynamic, targets: ["FresnicaSDK"]),
    ],
    targets: [
        .binaryTarget(name: "FresnicaSDKFFI", path: "../FresnicaSDKFFI.xcframework"),
        .target(
            name: "FresnicaSDK",
            dependencies: ["FresnicaSDKFFI"],
            path: "Sources/FresnicaSDK"
        ),
    ]
)
SWIFT_PACKAGE

archive_swift_framework() {
  platform="$1"
  release_folder="$2"
  archive_path="$3"
  derived_data="$4"

  if ! (
    cd "$SWIFT_PACKAGE_DIR"
    xcodebuild archive \
      -scheme FresnicaSDK \
      -destination "generic/platform=${platform}" \
      -archivePath "$archive_path" \
      -derivedDataPath "$derived_data" \
      SKIP_INSTALL=NO \
      BUILD_LIBRARY_FOR_DISTRIBUTION=YES \
      ONLY_ACTIVE_ARCH=NO \
      IPHONEOS_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET"
  ); then
    echo "failed to archive FresnicaSDK for ${platform}" >&2
    echo "if Xcode reports no matching generic iOS destination, install the iOS platform" >&2
    echo "with Xcode > Settings > Components or: xcodebuild -downloadPlatform iOS" >&2
    exit 1
  fi

  framework="$archive_path.xcarchive/Products/usr/local/lib/FresnicaSDK.framework"
  if [ ! -d "$framework" ]; then
    echo "missing archived FresnicaSDK.framework for $platform" >&2
    exit 1
  fi

  modules="$derived_data/Build/Intermediates.noindex/ArchiveIntermediates/FresnicaSDK/BuildProductsPath/$release_folder/FresnicaSDK.swiftmodule"
  if [ ! -d "$modules" ]; then
    echo "missing archived FresnicaSDK Swift modules for $platform" >&2
    exit 1
  fi
  mkdir -p "$framework/Modules"
  rm -rf "$framework/Modules/FresnicaSDK.swiftmodule"
  cp -R "$modules" "$framework/Modules/FresnicaSDK.swiftmodule"
}

APPLE_ARCHIVES="$OUTPUT_DIR/apple-archives"
archive_swift_framework \
  "iOS" \
  "Release-iphoneos" \
  "$APPLE_ARCHIVES/device" \
  "$APPLE_ARCHIVES/device-derived"
archive_swift_framework \
  "iOS Simulator" \
  "Release-iphonesimulator" \
  "$APPLE_ARCHIVES/simulator" \
  "$APPLE_ARCHIVES/simulator-derived"

xcodebuild -create-xcframework \
  -framework "$APPLE_ARCHIVES/device.xcarchive/Products/usr/local/lib/FresnicaSDK.framework" \
  -framework "$APPLE_ARCHIVES/simulator.xcarchive/Products/usr/local/lib/FresnicaSDK.framework" \
  -output "$OUTPUT_DIR/FresnicaSDK.xcframework"

test -d "$OUTPUT_DIR/FresnicaSDK.xcframework"
rm -rf "$SWIFT_PACKAGE_DIR" "$APPLE_ARCHIVES"

printf 'Apple Native SDK package ready at %s\n' "$OUTPUT_DIR"

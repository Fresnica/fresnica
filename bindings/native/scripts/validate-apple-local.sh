#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
CRATE_DIR="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="${1:-$CRATE_DIR/build/apple}"
DEPLOYMENT_TARGET="${FRESNICA_IOS_DEPLOYMENT_TARGET:-13.4}"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "Apple Native SDK validation must run on macOS" >&2
  exit 1
fi

for tool in cargo rustup xcrun xcodebuild; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is required" >&2
    exit 1
  fi
done

bash "$SCRIPT_DIR/build-apple.sh" "$OUTPUT_DIR"

SWIFT_SOURCE="$OUTPUT_DIR/generated-swift/FresnicaSDK.swift"
FFI_HEADER="$OUTPUT_DIR/headers/FresnicaSDKFFI.h"
MODULEMAP="$OUTPUT_DIR/headers/module.modulemap"
SECURITY_STORE="$OUTPUT_DIR/platform-security/FresnicaWalletUnlockKeyStore.swift"
SECURITY_AUTH="$OUTPUT_DIR/platform-security/FresnicaSignerAuthorization.swift"
FFI_XCFRAMEWORK="$OUTPUT_DIR/FresnicaSDKFFI.xcframework"
SDK_XCFRAMEWORK="$OUTPUT_DIR/FresnicaSDK.xcframework"

for file in \
  "$SWIFT_SOURCE" \
  "$FFI_HEADER" \
  "$MODULEMAP" \
  "$SECURITY_STORE" \
  "$SECURITY_AUTH"; do
  test -s "$file"
done
for directory in "$FFI_XCFRAMEWORK" "$SDK_XCFRAMEWORK"; do
  test -d "$directory"
done

SDK="$(xcrun --sdk iphonesimulator --show-sdk-path)"

# Verify the generated API and SDK-owned platform authorization source agree before packaging.
xcrun --sdk iphonesimulator swiftc \
  -target "arm64-apple-ios${DEPLOYMENT_TARGET}-simulator" \
  -sdk "$SDK" \
  -I "$OUTPUT_DIR/headers" \
  -typecheck \
  "$SWIFT_SOURCE" \
  "$SECURITY_STORE" \
  "$SECURITY_AUTH"

SDK_FRAMEWORK="$(find "$SDK_XCFRAMEWORK" -type d -name FresnicaSDK.framework -path '*simulator*' -print -quit)"
FFI_HEADERS="$(find "$FFI_XCFRAMEWORK" -type d -name Headers -path '*simulator*' -print -quit)"

test -n "$SDK_FRAMEWORK"
test -n "$FFI_HEADERS"
test -d "$SDK_FRAMEWORK/Modules/FresnicaSDK.swiftmodule"

cat > "$OUTPUT_DIR/consumer-smoke.swift" <<'SWIFT'
import FresnicaSDK

let api: FresnicaSdkApiProtocol = FresnicaSdkApi()
let version = api.version()
precondition(version.nativeBindingApiVersion >= 1)
precondition(version.sdkApiVersion >= 1)
precondition(version.coreClientApiVersion >= 1)
SWIFT

# This proves an ordinary consumer can import the compiled public module without compiling
# generated SDK/security sources in its own target.
xcrun --sdk iphonesimulator swiftc \
  -target "arm64-apple-ios${DEPLOYMENT_TARGET}-simulator" \
  -sdk "$SDK" \
  -F "$(dirname "$SDK_FRAMEWORK")" \
  -I "$FFI_HEADERS" \
  -typecheck "$OUTPUT_DIR/consumer-smoke.swift"

if grep -R -q -E 'ReactNative|RCTBridge|Flutter' "$SDK_XCFRAMEWORK"; then
  echo "framework-specific adapter code leaked into FresnicaSDK.xcframework" >&2
  exit 1
fi

printf 'Fresnica Apple Native SDK validation: OK\n'
printf '  SDK: %s\n' "$SDK_XCFRAMEWORK"
printf '  FFI: %s\n' "$FFI_XCFRAMEWORK"

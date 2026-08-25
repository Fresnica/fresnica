# Fresnica Native SDK Packaging

This directory builds framework-neutral native SDK artifacts above `fresnica-sdk`.

The Native SDK contains the stable Swift/Kotlin-facing Fresnica API and Rust FFI library. React Native, Flutter and other framework adapters are separate products and must not be embedded here.

## Android

Supported baseline:

| Android ABI | Rust target |
| --- | --- |
| `armeabi-v7a` | `armv7-linux-androideabi` |
| `x86` | `i686-linux-android` |
| `x86_64` | `x86_64-linux-android` |
| `arm64-v8a` | `aarch64-linux-android` |

Minimum Android API: 26.

Build the Rust libraries and generated Kotlin:

```sh
cargo install cargo-ndk --version 4.1.2 --locked
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/26.1.10909125"
bash bindings/native/scripts/build-android.sh
```

Then build the AAR:

```sh
gradle -p bindings/native/platform/android assembleRelease
```

The resulting AAR contains:

- generated `com.fresnica.sdk` Kotlin API;
- `libfresnica_native_sdk.so` for all four ABIs;
- no React Native or Flutter adapter classes.

An ordinary Android consumer links this AAR. It does not build Rust or run UniFFI.

## Apple

Supported initial iOS slices:

- `aarch64-apple-ios`;
- `aarch64-apple-ios-sim`;
- `x86_64-apple-ios`.

Initial deployment baseline remains iOS 13.4 while the generalized package is validated against the existing mobile integration baseline.

Run on macOS:

```sh
bash bindings/native/scripts/build-apple.sh
```

Output:

```text
bindings/native/build/apple/
  device/libfresnica_native_sdk.a
  simulator/libfresnica_native_sdk.a
  generated-swift/
    FresnicaSDK.swift
    FresnicaSDKFFI.h
    FresnicaSDKFFI.modulemap
  headers/
    FresnicaSDKFFI.h
    module.modulemap
  FresnicaSDKFFI.xcframework/
```

The XCFramework contains the compiled Rust FFI library for device and simulator. `FresnicaSDK.swift` is the stable generated Swift API shipped alongside that binary package; consumers do not run Rust or UniFFI generation. A later release-packaging step may wrap these pieces into the final distribution container without changing the SDK semantic contract.

## Security and framework boundary

The native packages may expose low-level native operations required by trusted native platform code, including routine `WalletUnlockKey` signing operations. Framework adapters must continue to expose only the reviewed higher-level framework API and must not forward routine unlock keys or raw signing primitives into JavaScript/Dart.

Reveal/Export remains fresh-passcode-only. Protected envelopes remain opaque. Framework separation is a packaging change, not a change to Core security semantics.

## CI

`.github/workflows/native-sdk-bindings.yml` verifies the generalized UniFFI contract and generated Swift/Kotlin symbols.

`.github/workflows/native-sdk-platform-packaging.yml` builds and validates the Android AAR and Apple package and rejects obvious framework-specific code leakage into the Native SDK artifacts.

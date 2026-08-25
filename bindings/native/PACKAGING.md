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
- `com.fresnica.sdk.security` Keystore / native signer-authorization helpers;
- `libfresnica_native_sdk.so` for all four ABIs;
- no React Native or Flutter adapter classes.

An ordinary Android consumer links this AAR. It does not build Rust or run UniFFI.

## Apple

Supported Apple slices:

- iOS device: `aarch64-apple-ios`;
- iOS simulator: `aarch64-apple-ios-sim`, `x86_64-apple-ios`;
- macOS: `aarch64-apple-darwin`, `x86_64-apple-darwin`.

Deployment baselines:

- iOS 13.4, retained from the validated mobile integration baseline;
- macOS 12.0 for the first desktop Swift package.

Run on macOS:

```sh
bash bindings/native/scripts/build-apple.sh
```

For the full direct-consumer validation, including a compiled-module import smoke check:

```sh
bash bindings/native/scripts/validate-apple-local.sh
```

Output:

```text
bindings/native/build/apple/
  device/libfresnica_native_sdk.a
  simulator/libfresnica_native_sdk.a
  macos/libfresnica_native_sdk.a
  generated-swift/
    FresnicaSDK.swift
    FresnicaSDKFFI.h
    FresnicaSDKFFI.modulemap
  headers/
    FresnicaSDKFFI.h
    module.modulemap
  platform-security/
    FresnicaWalletUnlockKeyStore.swift
    FresnicaSignerAuthorization.swift
  FresnicaSDKFFI.xcframework/
  FresnicaSDK.xcframework/
```

`FresnicaSDKFFI.xcframework` is the low-level Rust FFI package. It now carries iOS-device, iOS-simulator and universal macOS slices. The build then uses a temporary Swift package only as a packaging mechanism to compile the generated API plus `platform-security/` into an importable `FresnicaSDK.xcframework` for the same platforms. Consumers import `FresnicaSDK`; ordinary application builds do not compile Fresnica Rust, run UniFFI generation, or compile SDK-owned Swift sources. The loose generated source remains in the build output for inspection/conformance.

The final Swift framework is built with `BUILD_LIBRARY_FOR_DISTRIBUTION=YES` and packaged from separate iOS-device, iOS-simulator and macOS archives. `FresnicaSDKFFI.xcframework` remains in the distribution because the generated Swift module has a compile/link dependency on its FFI module.

`validate-apple-local.sh` rebuilds the package, type-checks the generated Swift API together with the SDK-owned Keychain/LocalAuthentication helpers for both iOS and macOS, then proves that separate iOS and macOS consumer sources can `import FresnicaSDK` from the compiled XCFramework without compiling those SDK sources themselves. The macOS Keychain path opts into the Data Protection Keychain explicitly while preserving the already-validated iOS behavior.

## Security and framework boundary

The native packages may expose low-level native operations required by trusted native platform code, including routine `WalletUnlockKey` signing operations. Framework adapters must continue to expose only the reviewed higher-level framework API and must not forward routine unlock keys or raw signing primitives into JavaScript/Dart.

Reveal/Export remains fresh-passcode-only. Protected envelopes remain opaque. Framework separation is a packaging change, not a change to Core security semantics.

## CI

`.github/workflows/native-sdk-bindings.yml` verifies the generalized UniFFI contract and generated Swift/Kotlin symbols.

`.github/workflows/native-sdk-platform-packaging.yml` builds and validates the Android AAR and Apple package and rejects obvious framework-specific code leakage into the Native SDK artifacts.

## Desktop packaging boundary

Windows/Linux/macOS binary target shapes are still pending. Do not call a bare `.dll`/`.so` a complete SDK until the corresponding direct-consumer language surface is defined. The current UniFFI product surface is explicit for Kotlin and Swift; desktop packaging should reuse a supported generated binding/runtime or introduce a separately reviewed stable ABI rather than exposing UniFFI internals as an accidental public C API.

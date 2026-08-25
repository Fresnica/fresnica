# Fresnica Native SDK Release Contract

Status: **pre-1.0 direct-consumer SDK**.

This contract applies to the generalized `fresnica-native-sdk` product, not the transitional `mobile-sdk-v0.1.0` package. The Native SDK release contains framework-neutral Android/Apple binaries only. React Native, Flutter and application product code are released/compiled separately from the Native SDK.

## Version and tag

The release version is the package version in `bindings/native/Cargo.toml`.

Release tags use:

```text
native-sdk-vMAJOR.MINOR.PATCH
```

Pre-1.0 consumers must pin an exact release. `NATIVE_BINDING_API_VERSION`, `SDK_API_VERSION` and `CLIENT_API_VERSION` remain independent runtime/API compatibility versions and are recorded in the release manifest; changing one of those constants requires explicit compatibility review even when the package version also changes.

A release is immutable. The release workflow refuses to replace an existing tag/release. A correction requires a new package version and marker.

## Release intent

Publishing requires a reviewed marker:

```text
releases/native-sdk-vVERSION.json
```

The marker is the only `main`-push release trigger. Ordinary pushes to `main` do not build or publish Native SDK platform packages.

Required marker shape:

```json
{
  "kind": "fresnica-native-sdk-release",
  "version": "0.1.0",
  "channel": "preview",
  "native_binding_api_version": 1,
  "sdk_api_version": 2,
  "core_client_api_version": 2,
  "android": {
    "min_sdk": 26,
    "abis": ["armeabi-v7a", "x86", "x86_64", "arm64-v8a"]
  },
  "apple": {
    "minimum_ios": "13.4",
    "minimum_macos": "12.0"
  }
}
```

Do not add a marker merely to test the workflow. Pull requests can validate the workflow/package contract without a marker, and `workflow_dispatch` requires an existing marker before it will publish.

## Binary artifacts

A Native SDK release publishes exactly these consumer-facing files:

```text
fresnica-native-sdk-VERSION.aar
FresnicaSDK-VERSION-apple.zip
fresnica-native-sdk-VERSION-manifest.json
SHA256SUMS
```

### Android

`fresnica-native-sdk-VERSION.aar` contains:

- generated `com.fresnica.sdk` Kotlin API;
- native signer-authorization/Keystore helpers under `com.fresnica.sdk.security`;
- `libfresnica_native_sdk.so` for all four supported ABIs;
- no React Native or Flutter implementation.

The release workflow uses the same `build-android.sh` + AAR validation path as Native SDK platform packaging.

### Apple

`FresnicaSDK-VERSION-apple.zip` contains:

```text
FresnicaSDK.xcframework/
FresnicaSDKFFI.xcframework/
```

`FresnicaSDK.xcframework` is the direct Swift consumer module. `FresnicaSDKFFI.xcframework` remains beside it because the generated Swift module links/imports the UniFFI FFI module. Both XCFrameworks carry iOS and macOS slices. SDK-owned Keychain/LocalAuthentication code is already compiled into `FresnicaSDK.xcframework`; loose generated/security Swift sources are build diagnostics, not release inputs.

The Apple release job must run `bindings/native/scripts/validate-apple-local.sh` before staging the zip. The complete iOS direct-consumer path passed on real macOS/Xcode on 2026-08-25; the new macOS slice must pass the same expanded validator before the first generalized Native SDK release marker is added.

React Native adapter binaries are deliberately absent. A framework consumer builds the canonical adapter once in its own framework environment according to `mobile-framework-adapter-contract.md`.

## Manifest

The release manifest starts from the reviewed marker and adds immutable build identity:

- Git commit SHA;
- release tag;
- Native/SDK/Core package versions read from source;
- artifact filenames.

`SHA256SUMS` covers the AAR, Apple zip and final manifest.

## CI / publishing policy

`.github/workflows/native-sdk-release.yml` has three modes:

- pull request: validate version/API/marker contract only; no heavy platform builds;
- `workflow_dispatch`: explicit publish attempt for a version that already has a marker;
- `main` push changing `releases/native-sdk-v*.json`: build Android + Apple, validate, checksum and create a prerelease.

This keeps expensive Android/Xcode packaging off ordinary `main` pushes while preserving a reproducible release path.

The existing `mobile-sdk-release.yml` remains frozen as the transitional Mobile v0.1.0 release path. Do not reuse its tag namespace or artifact shape for generalized Native SDK releases.

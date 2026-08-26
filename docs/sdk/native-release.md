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
  "version": "0.2.1",
  "channel": "preview",
  "native_binding_api_version": 2,
  "sdk_api_version": 3,
  "core_client_api_version": 3,
  "android": {
    "min_sdk": 26,
    "abis": ["armeabi-v7a", "x86", "x86_64", "arm64-v8a"],
    "host_dependencies": [
      "org.jetbrains.kotlin:kotlin-stdlib:1.9.24",
      "net.java.dev.jna:jna:5.12.1@aar",
      "androidx.annotation:annotation:1.8.2"
    ]
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

The GitHub release artifact is a raw AAR, not a Maven publication, so Gradle cannot discover transitive dependencies from a POM. A direct local-file consumer must also provide the dependencies recorded in the release manifest. For the current 0.2.x baseline:

```gradle
dependencies {
    implementation files("libs/fresnica-native-sdk-VERSION.aar")
    implementation "org.jetbrains.kotlin:kotlin-stdlib:1.9.24"
    implementation "net.java.dev.jna:jna:5.12.1@aar"
    implementation "androidx.annotation:annotation:1.8.2"
}
```

JNA is required by the UniFFI Kotlin runtime. Fresnica intentionally does not build a fat AAR just to hide these dependencies. A future Maven publication may carry the same dependency contract in POM/module metadata without changing the Native SDK API.

Before staging the release AAR, CI compiles `bindings/native/tests/android-consumer` through `validate-android-consumer.sh` against the raw AAR and the marker-declared dependency set. This is the direct-consumer acceptance check for the GitHub-file distribution shape.

### Apple

`FresnicaSDK-VERSION-apple.zip` contains:

```text
FresnicaSDK.xcframework/
FresnicaSDKFFI.xcframework/
```

`FresnicaSDK.xcframework` is the direct Swift consumer module. `FresnicaSDKFFI.xcframework` remains beside it because the generated Swift module links/imports the UniFFI FFI module. Both XCFrameworks carry iOS and macOS slices. SDK-owned Keychain/LocalAuthentication code is already compiled into `FresnicaSDK.xcframework`; loose generated/security Swift sources are build diagnostics, not release inputs.

The Apple release job runs `bindings/native/scripts/validate-apple-local.sh` before staging the zip. The complete iOS + macOS direct-consumer path passed on real macOS/Xcode on 2026-08-25, including independent Swift consumer import/typecheck. That gate cleared the first generalized Native SDK release marker.

React Native adapter binaries are deliberately absent. A framework consumer builds the canonical adapter once in its own framework environment according to `docs/platforms/mobile/framework-adapter.md`.

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

The old `mobile-sdk-v0.1.0` release is a frozen legacy compatibility artifact. Its release workflow is preserved in Git history/tagged source rather than kept as an active publisher on `main`. The generalized product line began with `native-sdk-v0.1.0` and uses the independent `native-sdk-v*` tag namespace. The current Mobile security/HD baseline is `native-sdk-v0.2.1` (Native package 0.2.1 / Native Binding API 2 / SDK API 3 / Core Client API 3). `native-sdk-v0.2.0` established the new UX/security contract but is superseded by v0.2.1, which fixes device-domain commit/cleanup failure semantics and carries the aligned Mobile handoff documentation.

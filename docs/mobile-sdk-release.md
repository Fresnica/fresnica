# Fresnica Mobile SDK Release Contract

Status: **integration-stable pre-1.0 SDK**.

The Fresnica Mobile SDK is released independently from the future `fresnica-mobile` application. The SDK owns Core/mobile binding/native security integration; the Mobile app owns React Native product state, Realm lifecycle, screens, network behavior and application orchestration.

## Versioning

Release tags use:

```text
mobile-sdk-vMAJOR.MINOR.PATCH
```

The first integration release is:

```text
mobile-sdk-v0.1.0
```

`0.x` means the SDK is suitable for pinned integration but does not claim a frozen 1.0 compatibility promise. Breaking binding/native-host changes require an explicit SDK version bump and corresponding API-version review.

The release also records two runtime contract versions:

- `MOBILE_BINDING_API_VERSION`
- `CLIENT_API_VERSION`

The Mobile application should query/report these versions in diagnostics and reject an SDK whose contract version is outside the app's supported range.

## 0.1.0 compatibility baseline

- Fresnica Core crate: `0.1.0`
- Fresnica mobile facade crate: `0.1.0`
- Mobile binding API: `2`
- Core client API: `2`
- UniFFI: `0.32.0`
- Android minSdk: `26`
- Android compileSdk: `34`
- Android native ABIs: `armeabi-v7a`, `x86`, `x86_64`, `arm64-v8a`
- Apple deployment target: iOS `13.4`
- React Native adapter implementation: conventional/legacy native module
- Android RN compile-only baseline: React Native `0.74.2`

The RN compile baseline is **not** a statement that the adapter has already been validated against React Native 0.87. New-Architecture/TurboModule migration is governed by `docs/react-native-upgrade-playbook.md`; Core/UniFFI/security contracts must remain unchanged during that migration.

## Release artifacts

A Mobile SDK release publishes:

```text
fresnica-mobile-sdk-VERSION.aar
FresnicaMobileSDK-VERSION-apple.zip
fresnica-mobile-sdk-VERSION-manifest.json
SHA256SUMS
```

### Android AAR

The AAR contains:

- generated UniFFI Kotlin API;
- Fresnica Android native authorization / WalletUnlockKey implementation;
- `FresnicaCoreModule` and `FresnicaCorePackage` React Native adapter;
- Rust `libfresnica_mobile_core.so` for all four supported ABIs.

The AAR deliberately does not embed React Native itself. A host using the AAR must provide compatible host dependencies, including React Native and the Android dependencies declared by the SDK module (`JNA`, AndroidX annotation and AndroidX biometric).

### Apple package

The Apple zip contains:

- `FresnicaCoreFFI.xcframework`;
- generated `FresnicaCore.swift`;
- `FresnicaWalletUnlockKeyStore.swift`;
- `FresnicaSignerAuthorization.swift`;
- `FresnicaCoreModule.swift`;
- `FresnicaCoreModule.m`.

The host application adds the XCFramework and Swift/Objective-C adapter sources to its target. React Native headers are supplied by the host project.

A future Swift Package may wrap this layout, but adding SwiftPM packaging must not change the Core/mobile API contract.

## What is not in the SDK release

The SDK release does not package the application-side donor TypeScript from PR #81-#84:

- wallet lifecycle persistence coordinator;
- Realm app schema/store adapter;
- account provisioning coordinator;
- signer Reveal/Export product coordinator.

Those belong in the independent Mobile application. See `docs/mobile-app-migration-pr81-pr84.md` for absorption guidance.

## Release marker

A reviewed file under:

```text
releases/mobile-sdk-vVERSION.json
```

is the release intent. When a new marker is merged to `main`, the release workflow validates that the marker version matches the Cargo package version and API constants, builds both platform packages, computes checksums and creates the corresponding prerelease/tag.

The release is immutable. Rebuilding an already published tag should fail rather than silently replacing binaries. A corrected SDK requires a new version.

## Consumer rule for `fresnica-mobile`

The independent Mobile project should pin an exact release version during pre-1.0 development. Do not consume `main` Actions artifacts as a normal dependency.

Recommended dependency record in the Mobile repository:

```text
FRESNICA_MOBILE_SDK_VERSION=0.1.0
MOBILE_BINDING_API_VERSION=2
CORE_CLIENT_API_VERSION=2
```

The app may update those values only in a dedicated SDK-upgrade change that runs native host integration tests.

## Release acceptance criteria

A release is publishable only when:

- Rust Core and mobile facade contracts are versioned;
- Android AAR builds and lint passes;
- the AAR contains all four Rust ABI libraries;
- the AAR contains the Fresnica RN native module/package;
- Apple Core XCFramework builds for device and simulator;
- generated Swift plus native authorization/module source type-checks;
- React Native does not receive `WalletUnlockKey` or raw signing APIs;
- release manifest/version constants agree;
- SHA-256 checksums are produced for the downloadable artifacts.

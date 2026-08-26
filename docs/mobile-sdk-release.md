# Fresnica Mobile SDK Release Contract

Status: **frozen historical contract for `mobile-sdk-v0.1.0` only**.

This document records the transitional `mobile-sdk-v0.1.0` release that preceded the generalized Native-SDK/framework-adapter split. It is retained for audit/migration reference only. New Mobile integrations use the current generalized Native SDK baseline from `docs/mobile-sdk-usage.md` (`native-sdk-v0.2.1` at the time of this update).

The finalized framework boundary is defined in `docs/mobile-framework-adapter-contract.md`; the generalized binary release policy is `docs/native-sdk-release.md`. Do not add new `mobile-sdk-v*` releases. The old release workflow is kept in Git history/tagged source, not as an active `main` publisher.

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

`mobile-sdk-v0.1.0` predates the finalized Native-SDK/framework-adapter packaging split. Its artifact layout below is therefore a **transitional integration baseline**, not the target layout for future releases.

## v0.1.0 release artifacts

The v0.1.0 Mobile SDK release publishes:

```text
fresnica-mobile-sdk-VERSION.aar
FresnicaMobileSDK-VERSION-apple.zip
fresnica-mobile-sdk-VERSION-manifest.json
SHA256SUMS
```

### Android AAR

The v0.1.0 AAR contains:

- generated UniFFI Kotlin API;
- Fresnica Android native authorization / WalletUnlockKey implementation;
- `FresnicaCoreModule` and `FresnicaCorePackage` React Native adapter;
- Rust `libfresnica_mobile_core.so` for all four supported ABIs.

The AAR deliberately does not embed React Native itself. A host using the AAR must provide compatible host dependencies, including React Native and the Android dependencies declared by the SDK module (`JNA`, AndroidX annotation and AndroidX biometric).

### Apple package

The v0.1.0 Apple zip contains:

- `FresnicaCoreFFI.xcframework`;
- generated `FresnicaCore.swift`;
- `FresnicaWalletUnlockKeyStore.swift`;
- `FresnicaSignerAuthorization.swift`;
- `FresnicaCoreModule.swift`;
- `FresnicaCoreModule.m`.

The host application adds the XCFramework and Swift/Objective-C adapter sources to its target. React Native headers are supplied by the host project.

## Replacement release shape

The generalized release line now separates native binaries from framework adapters:

```text
Native SDK release
  Android: fresnica-native-sdk-VERSION.aar
  Apple:   FresnicaSDK-VERSION-apple.zip
  manifest + checksums

Source tree / adapter package
  canonical React Native adapter source
  canonical Flutter adapter source (future)
  adapter build tooling + conformance tests
```

The Native SDK binary must not contain React Native, Flutter, or other framework-specific code. A native Android/iOS consumer uses the Native SDK directly.

React Native/Flutter consumers follow `docs/mobile-framework-adapter-contract.md`: compile the canonical adapter once against the consumer's framework/toolchain, store the generated adapter binary and compatibility manifest, and do not compile adapter source during ordinary app builds.

A future Swift Package or Maven publication may wrap the native binary layout, but packaging convenience must not change the Core/mobile API contract.

## What is not in the SDK release

The SDK release does not package the application-side donor TypeScript from PR #81-#84:

- wallet lifecycle persistence coordinator;
- Realm app schema/store adapter;
- account provisioning coordinator;
- signer Reveal/Export product coordinator.

Those belong in the independent Mobile application. See `docs/mobile-app-migration-pr81-pr84.md` for absorption guidance.

Framework adapter source is also not application product logic. It remains Fresnica-owned canonical glue and is compiled by the consuming framework project according to `docs/mobile-framework-adapter-contract.md`.

## Historical release marker

`releases/mobile-sdk-v0.1.0.json` is retained as the immutable release record for the transitional artifact. No new `mobile-sdk-v*` markers should be created. New releases use `releases/native-sdk-vVERSION.json`.

## Consumer rule for `fresnica-mobile`

The independent Mobile project must not pin this legacy Mobile SDK. Use the generalized dependency record from `docs/mobile-sdk-usage.md` (current baseline `native-sdk-v0.2.1`, Native Binding API 2, SDK API 3, Core Client API 3).

For React Native, Mobile must additionally record the generated adapter compatibility manifest defined in `docs/mobile-framework-adapter-contract.md`. A stale framework/Binding-API combination should fail CI with an adapter-rebuild requirement; CI must not silently rebuild the adapter on every application build.

## Release acceptance criteria

For the current v0.1.0 transitional release, publishability requires:

- Rust Core and mobile facade contracts are versioned;
- Android AAR builds and lint passes;
- the AAR contains all four Rust ABI libraries;
- the AAR contains the Fresnica RN native module/package;
- Apple Core XCFramework builds for device and simulator;
- generated Swift plus native authorization/module source type-checks;
- React Native does not receive `WalletUnlockKey` or raw signing APIs;
- release manifest/version constants agree;
- SHA-256 checksums are produced for the downloadable artifacts.

Once the packaging split is implemented, acceptance criteria must instead verify that Native SDK artifacts contain no framework-specific adapter implementation, while adapter source/build tooling has its own compatibility and conformance checks.

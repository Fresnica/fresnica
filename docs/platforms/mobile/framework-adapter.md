# Mobile Native SDK and Framework Adapter Contract

Status: **foundational integration contract for Fresnica Mobile and other application consumers**.

This document fixes the boundary between the Fresnica Native SDK and application-framework adapters. It is the baseline for integrating Fresnica into React Native, Flutter, or future application frameworks.

## Decision

Fresnica publishes:

1. a **compiled Native SDK** for each supported native platform; and
2. **canonical framework-adapter source/reference implementation**.

A consuming application compiles the adapter **once in its own framework/toolchain environment**, stores the generated adapter binary, and uses that binary for normal application builds.

The adapter is **not** rebuilt on every application build.

```text
Fresnica
  |
  +-- Android Native SDK binary
  +-- Apple Native SDK binary
  +-- React Native adapter source
  +-- Flutter adapter source (future)
        |
        | one-time adapter build in consumer environment
        v
Consumer project
  +-- pinned Native SDK binary
  +-- generated framework adapter binary
        |
        v
  normal application builds
```

## Stable ownership boundary

### Fresnica owns

- Rust Core and cryptographic/signing behavior;
- FFI-neutral mobile API and binding contract;
- generated/native platform API used by adapters;
- Android native SDK binary;
- Apple native SDK binary;
- Keychain/Keystore and native signer-authorization implementation, shipped as native platform support rather than framework glue;
- stable error categories and API-version reporting;
- canonical React Native adapter source;
- future canonical Flutter/other framework adapter source;
- adapter build recipe/tooling and compatibility checks;
- conformance tests for the adapter boundary.

### Consumer application owns

- the application framework version, for example React Native 0.87;
- application Android/iOS toolchain versions;
- running the adapter build during initial integration or a required upgrade;
- storing/versioning the generated adapter binaries;
- linking the Native SDK and generated adapter binaries into normal app builds;
- application state, persistence, navigation, screens, network behavior and product orchestration.

### Toolchain-neutral adapter rule

The framework adapter must consume the application's toolchain rather than bring its own. In particular, canonical adapter source must not pin the consumer's Gradle, Android Gradle Plugin, Kotlin plugin, JDK, CocoaPods, or Xcode version merely because Fresnica CI uses a specific reference environment.

Fresnica may declare capability constraints that are part of the real integration contract, such as `minSdk 26`, AndroidX, an exact framework version for a reproducible adapter binary, required host dependencies, or a minimum compiler/platform capability when the source genuinely depends on it. It may also publish a tested compatibility matrix. A tested version is evidence, not a requirement unless the underlying capability makes it one.

For Android, the canonical build tool runs the consumer project's own `android/gradlew` and injects the adapter as a temporary subproject. Plugin resolution, repositories, `compileSdk`, AGP/Kotlin behavior, and other build-wide policy therefore come from the consumer project.

## Native SDK rule

The Native SDK is a **binary product**. A normal consuming project must not build Rust Core or UniFFI as part of its application build.

Target release shape:

```text
Android
  fresnica-native-sdk-VERSION.aar

Apple
  FresnicaSDK-VERSION-apple.zip
    -> FresnicaSDK.xcframework
    -> FresnicaSDKFFI.xcframework
```

The Native SDK must not contain React Native, Flutter, or other framework-specific adapter code.

A native Android/iOS application can consume the Native SDK directly without any framework adapter.

## Framework adapter rule

A framework adapter is thin integration glue between a framework runtime and the Fresnica Native SDK.

It may contain:

- argument/result conversion;
- Promise/Future/callback conversion;
- framework lifecycle glue;
- thread/queue dispatch required by the framework;
- framework registration/autolinking glue;
- stable Fresnica error conversion;
- platform UI/lifecycle glue required to drive an SDK-owned authorization primitive (for example authenticating the exact Android `Cipher` returned by the Native SDK helper).

It must not own or duplicate:

- cryptography;
- secret/mnemonic validation;
- signer identity verification;
- protected-envelope parsing/mutation;
- transaction hashing/signing logic;
- WalletUnlockKey handling policy;
- Keychain/Keystore security policy or credential storage implementation.

The rule is:

> **Adapter = mechanical framework glue. Native SDK/Core = security authority.**

## One-time adapter build model

For React Native, the desired consumer flow is:

```text
Initial Mobile integration
  1. Pin Fresnica Native SDK version.
  2. Pin a Fresnica adapter source version/commit compatible with its Binding API.
  3. Detect the Mobile project's React Native/native toolchain versions.
  4. Compile the canonical RN adapter source against that environment.
  5. Produce Android/iOS adapter binaries.
  6. Store those binaries plus a compatibility manifest in the Mobile project
     or its controlled artifact store.
  7. Remove adapter source from the normal application compilation path.

Normal Mobile development
  -> link Native SDK binary
  -> link generated RN adapter binary
  -> do not rebuild Rust/Core
  -> do not rebuild the RN adapter
```

Representative generated outputs:

```text
vendor/fresnica/
  native/
    fresnica-native-sdk-VERSION.aar
    FresnicaSDK.xcframework
    FresnicaSDKFFI.xcframework
  adapter/
    react-native/
      fresnica-rn-adapter.aar
      FresnicaRNAdapter.xcframework
      adapter-manifest.json
```

The exact consumer directory names are not contractual. The binary/rebuild behavior is.

## When an adapter must be rebuilt

Rebuild the adapter when at least one of these changes crosses a compatibility boundary:

1. the consumer changes React Native/Flutter framework version and the existing adapter binary is not declared compatible;
2. `NATIVE_BINDING_API_VERSION` changes incompatibly;
3. canonical adapter source changes because framework glue must change;
4. a native platform/toolchain change creates an actual binary/API incompatibility requiring a new adapter build.

Do **not** rebuild the adapter merely because:

- normal application source changed;
- screens/navigation/business logic changed;
- Realm/application persistence changed;
- Rust/Core implementation changed while the consumed Native SDK/Binding API remains the same;
- the app is doing a normal debug/release build.

A framework upgrade should therefore normally be:

```text
React Native old -> new
  -> run Fresnica adapter build once
  -> validate
  -> store new adapter binaries/manifest
  -> resume normal builds
```

## Compatibility manifest

Every generated adapter binary set should have a machine-readable manifest. For example, the latest published 0.2.1 baseline records:

```json
{
  "framework": "react-native",
  "frameworkVersion": "0.87.0",
  "fresnicaNativeSdkVersion": "0.2.1",
  "nativeBindingApiVersion": 2,
  "adapterSourceVersion": "0.2.1"
}
```

The adapter build tool may additionally record relevant Android/iOS compiler/toolchain versions.

Normal Mobile CI should verify that the checked-in/configured environment matches this manifest. If not, CI should fail with a clear **adapter rebuild required** message rather than silently rebuilding it.

## Required tooling goal

Fresnica should provide a small adapter-build entry point so consumers do not manually wire Gradle/Xcode steps.

Conceptual interface:

```text
fresnica-adapter build react-native
```

The tool should:

- read the consumer framework version;
- validate the pinned Native SDK and Binding API;
- fetch/use the matching canonical adapter source;
- compile Android and Apple adapter binaries in the consumer's environment;
- emit the binaries and compatibility manifest;
- run adapter conformance checks;
- leave normal app builds consuming binaries only.

Flutter and future framework support should follow the same model rather than introducing a second architecture.

## Security boundary exposed to frameworks

Framework APIs may expose the approved high-level mobile operations, but must continue to prevent routine JavaScript/Dart access to native-only signing material and low-level unlock primitives.

In particular:

- `WalletUnlockKey` remains native-only;
- raw routine `signTransactionXdr` remains outside the framework API;
- routine protected-software signing remains native-only;
- Reveal/Export requires a fresh app passcode;
- protected envelopes remain opaque to application/framework code.

These constraints are independent of React Native/Flutter versions and must survive every adapter rebuild.

## Migration status

The Native-SDK/framework-adapter split is now the authoritative Mobile integration model. `mobile-sdk-v0.1.0` is frozen as a legacy compatibility baseline and must not be used for new Mobile work.

The generalized line is:

```text
Native SDK release
  = compiled Android / Apple platform content only

Framework adapter
  = canonical source + one-time consumer build tooling

Consumer application
  = pinned Native SDK + generated adapter binaries + compatibility manifest
```

The first generalized release was `native-sdk-v0.1.0`. The latest published Mobile security/HD baseline is `native-sdk-v0.2.1`: Native Binding API 2 over SDK API 3 / Core Client API 3, with React Native adapter source 0.2.1. Adapter 0.2.1 is a build-tooling compatibility patch: it keeps the same framework/API surface while removing Fresnica-owned Android Gradle/AGP/Kotlin version selection. v0.2 intentionally changes the framework/native contract to add mnemonic-source HD derivation and the device-level System Auth Protection Domain. Compatibility remains explicit through `NATIVE_BINDING_API_VERSION` and the adapter manifest.


The current development source is 0.3.0 / Native Binding API 3 / SDK API 5 / Core Client API 5 with React Native adapter source 0.3.0. It adds `signMessageWithSystemAuth` and `signMessageWithPasscode` for SEP-53 dapp challenges. This source version must not be treated as a published binary baseline until the matching Native SDK release exists.

The transitional `bindings/mobile` source is retired from `main`; #81-#84 semantics remain migration acceptance criteria and history remains in Git/tags.

## Mobile onboarding baseline

The independent `fresnica-mobile` project should use this document as its integration starting point:

1. choose and pin its React Native version;
2. pin `native-sdk-v0.2.1` / Native Binding API 2 and matching adapter source 0.2.1;
3. generate its RN adapter binaries once from the matching canonical adapter source;
4. store the adapter binaries and manifest under Mobile ownership;
5. run a native/framework smoke test such as `parseAccount`;
6. establish the one app passcode and optional one-time device System Auth Domain;
7. only then absorb the application-side lifecycle/persistence flows described in `docs/platforms/mobile/app-migration-pr81-pr84.md`.

The Mobile project must not make Rust/Core/UniFFI or adapter-source compilation part of ordinary application builds.

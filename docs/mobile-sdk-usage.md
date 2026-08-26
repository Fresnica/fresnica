# Fresnica Mobile SDK 使用指南

Status: **Mobile integration baseline — Native SDK v0.1.0**.

本文件是独立 `fresnica-mobile` 项目的开工入口。Mobile 不应复制或重新编译 Fresnica Rust Core；它消费已发布的 Native SDK，并在自己的 React Native/toolchain 环境中一次性编译 canonical React Native adapter。

## 1. 固定版本基线

首个 generalized Native SDK 基线：

- Native SDK release/tag: `native-sdk-v0.1.0`
- Release target commit: `c25e162a6d982c7b98658ac857630bbe59256a14`
- Native SDK package: `0.1.0`
- Native Binding API: `1`
- Universal SDK API: `2`
- Core Client API: `2`
- React Native adapter source: `0.1.0`
- Android: `minSdk 26`, `armeabi-v7a`, `x86`, `x86_64`, `arm64-v8a`
- Apple: iOS `13.4+`, macOS `12.0+`
- React Native real-consumer validation baseline: RN `0.87` with CocoaPods on macOS/Xcode

New Mobile work MUST NOT use the legacy `mobile-sdk-v0.1.0` artifact. That package is frozen compatibility history. `bindings/mobile` remains donor/reference material only until the independent Mobile project has absorbed the #81-#84 lifecycle behavior.

Pre-1.0 consumers should pin exact SDK and React Native versions. Do not use floating SDK versions or React Native semver ranges for the adapter build.

## 2. Mobile 消费的边界

```text
fresnica-mobile React Native app
        |
        v
FresnicaRNAdapter binary
        |
        v
Fresnica Native SDK binary
        |
        v
fresnica-sdk
        |
        v
Rust Core
```

Normal Mobile builds:

- link the pinned Native SDK binary;
- link the previously generated RN adapter binary;
- do not compile Rust Core;
- do not run UniFFI generation;
- do not compile RN adapter source again.

Fresnica owns cryptography, signer identity checks, protected-envelope semantics, transaction signing and native signer-authorization helpers. Mobile owns Realm, navigation, screens, network/Horizon state and product orchestration.

## 3. 获取 Native SDK

Release page:

```text
https://github.com/manran/fresnica/releases/tag/native-sdk-v0.1.0
```

Consumer-facing files are:

```text
fresnica-native-sdk-0.1.0.aar
FresnicaSDK-0.1.0-apple.zip
fresnica-native-sdk-0.1.0-manifest.json
SHA256SUMS
```

Verify the downloaded artifacts against `SHA256SUMS` before storing them in the Mobile repository/artifact store.

Recommended Mobile-owned layout:

```text
vendor/fresnica/
  native/
    fresnica-native-sdk-0.1.0.aar
    FresnicaSDK.xcframework
    FresnicaSDKFFI.xcframework
  adapter/
    react-native/
      fresnica-rn-adapter.aar
      FresnicaRNAdapter.xcframework
      adapter-manifest.json
```

Directory names are not contractual; pinning and binary/rebuild behavior are.

## 4. 先固定 React Native 版本

The canonical adapter tool reads the consumer project's `package.json`. `react-native` must be an exact version such as:

```json
{
  "dependencies": {
    "react-native": "0.87.0"
  }
}
```

Values such as `^0.87.0`, `~0.87.0`, `latest` or workspace ranges are rejected because they cannot identify a reproducible adapter binary.

## 5. Android Native SDK

Copy:

```text
fresnica-native-sdk-0.1.0.aar
```

into the Mobile-controlled native artifact location.

Because the GitHub release is a raw AAR rather than a Maven publication, the Android host must also declare the Native SDK runtime dependencies:

```gradle
dependencies {
    implementation files("path/to/fresnica-native-sdk-0.1.0.aar")
    implementation "org.jetbrains.kotlin:kotlin-stdlib:1.9.24"
    implementation "net.java.dev.jna:jna:5.12.1@aar"
    implementation "androidx.annotation:annotation:1.8.2"
}
```

The Android project must use AndroidX (`android.useAndroidX=true`).

The Native SDK AAR itself contains:

- generated `com.fresnica.sdk` Kotlin API;
- native `.so` libraries for all four supported ABIs;
- `com.fresnica.sdk.security.WalletUnlockKeyStore`;
- `com.fresnica.sdk.security.FresnicaSignerAuthorization`;
- no React Native/Flutter application code.

## 6. Apple Native SDK

Extract:

```text
FresnicaSDK-0.1.0-apple.zip
```

which contains:

```text
FresnicaSDK.xcframework
FresnicaSDKFFI.xcframework
```

Both are required. `FresnicaSDK.xcframework` is the Swift consumer module; it depends on the accompanying UniFFI FFI framework.

SDK-owned Keychain/LocalAuthentication signing support is already compiled into `FresnicaSDK.xcframework`. Mobile must not copy the generated SDK/security Swift source into its own framework layer.

## 7. 一次性构建 Android React Native adapter

Use the canonical adapter source from the same Fresnica source/tag as the pinned SDK:

```text
adapters/react-native
```

Build once inside the actual Mobile environment:

```sh
node adapters/react-native/tooling/fresnica-adapter.mjs \
  build react-native \
  --platform android \
  --project /path/to/fresnica-mobile \
  --native-android-aar /path/to/fresnica-native-sdk-0.1.0.aar \
  --out /path/to/fresnica-mobile/vendor/fresnica/adapter/react-native
```

Output:

```text
fresnica-rn-adapter.aar
adapter-manifest.json
```

The adapter AAR does not embed React Native or the Native SDK. The host supplies the dependencies recorded in `adapter-manifest.json`, currently including:

```text
com.facebook.react:react-android:<exact RN version>
androidx.biometric:biometric:1.1.0
androidx.core:core:1.12.0
net.java.dev.jna:jna:5.12.1@aar
androidx.annotation:annotation:1.8.2
```

The canonical Android package class is:

```text
com.fresnica.sdk.reactnative.FresnicaCorePackage
```

The binary adapter is not an npm-autolinked source package, so the host should register this package through its normal application-level native package registration mechanism.

## 8. 一次性构建 Apple React Native adapter

Prerequisites:

- macOS/Xcode;
- exact React Native version in `package.json`;
- Mobile iOS pods installed with `pod install`;
- extracted `FresnicaSDK.xcframework` and `FresnicaSDKFFI.xcframework`.

Build once:

```sh
node adapters/react-native/tooling/fresnica-adapter.mjs \
  build react-native \
  --platform apple \
  --project /path/to/fresnica-mobile \
  --native-apple-sdk-xcframework /path/to/FresnicaSDK.xcframework \
  --native-apple-ffi-xcframework /path/to/FresnicaSDKFFI.xcframework \
  --out /path/to/fresnica-mobile/vendor/fresnica/adapter/react-native
```

Output:

```text
FresnicaRNAdapter.xcframework
adapter-manifest.json
```

Link all three frameworks:

```text
FresnicaSDK.xcframework
FresnicaSDKFFI.xcframework
FresnicaRNAdapter.xcframework
```

Keep `-ObjC` in the Apple host linker flags so the React Native Objective-C registration shim is retained.

The real-consumer validator is:

```sh
bash adapters/react-native/apple/validate-consumer.sh /path/to/fresnica-mobile
```

It validates the adapter against the consumer's actual CocoaPods-installed React Native headers/frameworks rather than reconstructing RN header layouts manually.

## 9. Normal CI 只检查 compatibility，不重建 adapter

After the Android and Apple adapter binaries are stored, normal Mobile CI should run:

```sh
node adapters/react-native/tooling/adapter-manifest.mjs check \
  --project /path/to/fresnica-mobile \
  --manifest /path/to/fresnica-mobile/vendor/fresnica/adapter/react-native/adapter-manifest.json
```

If the pinned framework/Native SDK/binding contract or the stored artifact digest no longer matches, the command fails with:

```text
adapter rebuild required
```

Do not silently rebuild the adapter during ordinary app builds.

Rebuild it only when a real compatibility boundary changes, such as:

- React Native version changes and the current binary is no longer compatible;
- `NATIVE_BINDING_API_VERSION` changes incompatibly;
- canonical adapter source changes;
- Android/iOS toolchain changes create an actual binary incompatibility.

Normal screen, Realm, navigation or business-logic changes do not require an adapter rebuild.

## 10. 第一个 smoke test

The React Native module name is:

```text
FresnicaCore
```

Minimal JavaScript/TypeScript smoke test:

```ts
import { NativeModules } from 'react-native';

const { FresnicaCore } = NativeModules;

const identity = await FresnicaCore.parseAccount(
  'GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR',
);

// identity.kind === 'classic'
// identity.address === input address
// identity.publicKey === input address
```

Before implementing wallet screens, prove this path on both Android and iOS:

```text
React Native
  -> FresnicaRNAdapter
  -> Native SDK
  -> Rust Core
  -> result back to JS
```

## 11. React Native 暴露的高层能力

The canonical adapter exposes:

- `parseAccount`
- `protectSecret`
- `protectMnemonic`
- `generateMnemonic`
- `reprotect`
- `reveal`
- `prepareEd25519Signing`
- `applyEd25519Signature`
- `canEnrollSystemAuth`
- `hasSystemAuth`
- `removeSystemAuth`
- `enrollSystemAuth`
- `signWithSystemAuth`
- `signWithPasscode`

It intentionally does **not** expose routine low-level operations such as:

- `deriveUnlockKey`
- `validateUnlockKey`
- raw `signTransactionXdr`

`WalletUnlockKey`, Android biometric `Cipher` objects and equivalent native authorization state remain native-only.

## 12. Error contract

Core/SDK errors use stable categories:

```text
invalid-input
invalid-passcode
invalid-unlock-key
invalid-protected-data
identity-mismatch
invalid-transaction
core-error
```

Framework/native authorization may additionally surface errors such as system-auth unavailable/not enrolled, user cancellation, authentication failure, native integration failure or an already active authentication operation.

Product code should branch on stable error codes, not parse human-readable error messages.

## 13. Mobile persistence model

Mobile owns Realm/persistence. Preserve this conceptual graph:

```text
AccountRecord
  identity/address/network/product metadata

SignerRecord
  signer public identity
  signer kind/provider metadata
  opaque protected envelope when applicable

AccountSignerReference
  account <-> signer relationship
```

Mandatory invariants:

1. Account is not Signer; the relationship is not necessarily one-to-one.
2. Watch-only is derived from absence of an applicable local signer reference.
3. A Classic account may use a signer public key different from the account master key.
4. Direct master-key watch-only upgrade must use Core identity verification (`expectedSignerPublicKey`).
5. `C...` contract identities must not be treated as Ed25519 software signers.
6. Protected envelope JSON is opaque to Mobile.
7. Secret/mnemonic/`WalletUnlockKey` must never be persisted in Realm, Redux/navigation state, logs, analytics or crash reports.
8. Routine signing remains native-only.
9. Reveal/Export always requires a fresh Fresnica app passcode.
10. Global passcode rotation stages every re-protected signer first, then commits atomically, then invalidates/re-enrolls system auth.

## 14. Mobile 推荐迁移顺序

After the smoke test succeeds:

1. Define the Mobile-owned Realm schema/migrations for Account / Signer / Reference.
2. Absorb watch-only create, attach and downgrade semantics from `docs/mobile-app-migration-pr81-pr84.md`.
3. Absorb secret/mnemonic import and mnemonic generation provisioning.
4. Connect SDK-owned system-auth signing to the persisted signer records.
5. Add global app-passcode rotation.
6. Add explicit Reveal / Export UX.
7. Add network/Horizon ledger signer/threshold resolution so `hasLocalSigner` is not confused with actual on-chain authorization.
8. Continue product screens/navigation/portfolio/history/SDEX/SEP flows independently from Core architecture.

Do not copy the donor TypeScript class names blindly. Preserve the behavior and security invariants while integrating with the actual Mobile project structure.

## 15. Ownership boundary after Mobile starts

`fresnica` continues to own:

- Rust Core;
- `fresnica-sdk` semantic contract;
- Native SDK binary releases;
- native Keychain/Keystore signer authorization;
- canonical RN adapter source and build tooling;
- conformance vectors/tests and API compatibility versions.

`fresnica-mobile` owns:

- React Native application;
- Realm configuration and migrations;
- Account/Signer application persistence;
- navigation/screens/product state;
- network/Horizon state;
- wallet lifecycle UX;
- settings/passcode/reveal UX;
- product-level SEP/SDEX/history/portfolio orchestration.

Mobile should upgrade by pinning a newer Native SDK release and rebuilding the framework adapter only when its compatibility manifest says a rebuild is required. It should not fork Core into the application repository.

## 16. 开工验收

Mobile integration baseline is considered established when its CI proves:

- exact React Native version is pinned;
- `native-sdk-v0.1.0` artifacts are checksum-verified and stored/pinned;
- Android adapter binary is built once and stored;
- Apple adapter binary is built once and stored;
- `adapter-manifest.json` passes compatibility checking;
- `FresnicaCore.parseAccount` works from React Native on Android and iOS;
- normal Mobile builds do not build Rust/Core/UniFFI or adapter source;
- no secret/mnemonic/WalletUnlockKey is persisted in application state.

After these checks, Mobile product development does not need to wait for further Fresnica Core infrastructure work.

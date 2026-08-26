# Mobile / Native SDK Binding Architecture

Status: **generalized Native SDK is the authoritative Mobile integration surface**.

This document defines how Fresnica Core is consumed by the independent React Native Mobile application. The old `fresnica-mobile-core` / `mobile-sdk-v0.1.0` line is frozen as a compatibility and migration reference; new Mobile work must use `fresnica-native-sdk` plus the canonical React Native adapter.

## Authoritative stack

```text
React Native application
        |
canonical Fresnica RN adapter binary
        |
Swift / Kotlin Native SDK API
        |
fresnica-native-sdk (UniFFI)
        |
fresnica-sdk
        |
CoreClientApi
        |
Fresnica Core
```

The application does **not** compile Rust Core, `fresnica-sdk`, UniFFI, or adapter source during normal builds.

## Products and ownership

Fresnica publishes/owns:

- `fresnica-native-sdk-VERSION.aar` for Android;
- `FresnicaSDK-VERSION-apple.zip` containing `FresnicaSDK.xcframework` and `FresnicaSDKFFI.xcframework`;
- the stable `FresnicaSdkApi` Swift/Kotlin surface;
- native Keychain/Keystore signer-authorization helpers;
- canonical React Native adapter source under `adapters/react-native`;
- adapter build/compatibility tooling;
- Core/SDK/security semantics and conformance tests.

The independent Mobile application owns:

- React Native version and application toolchain;
- one-time compilation of the canonical adapter against that exact environment;
- checked-in/controlled adapter binaries and compatibility manifest;
- Realm schema/migrations and application persistence;
- network/Horizon behavior, screens, navigation and product orchestration.

## Version contract

The first generalized Native SDK line uses independent compatibility numbers:

```text
Native package version:       0.1.0
NATIVE_BINDING_API_VERSION:   1
SDK_API_VERSION:              2
CLIENT_API_VERSION:           2
RN adapter source version:    0.1.0
```

Mobile must pin an exact pre-1.0 Native SDK release and record the adapter manifest. A package-version update is not automatically an API break; the API constants are the machine-readable compatibility boundary.

## Native API

Kotlin package: `com.fresnica.sdk`

Swift module: `FresnicaSDK`

Primary object: `FresnicaSdkApi`

The native API exposes wallet/signer lifecycle and signing primitives backed by the platform-neutral SDK, including:

- `version`
- `parseAccount`
- `protectSecret`
- `protectMnemonic`
- `generateMnemonic`
- `reprotect`
- `deriveUnlockKey`
- `validateUnlockKey`
- `signTransactionXdr`
- `reveal`
- `prepareEd25519Signing`
- `applyEd25519Signature`

`deriveUnlockKey`, `validateUnlockKey` and raw routine `signTransactionXdr` are **native-only** primitives. The React Native adapter must not forward unlock-key material to JavaScript.

## React Native surface

The canonical JavaScript module remains `FresnicaCore`. It exposes the reviewed high-level surface:

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

The adapter performs only argument/result conversion, React Native registration/lifecycle work and the platform UI steps needed to drive SDK-owned native authorization.

It must not reimplement derivation, protected-envelope parsing, signer identity checks, transaction hashing/signing, signature verification, Keychain/Keystore policy, or `WalletUnlockKey` handling.

## Secret boundary

Routine software signing is:

```text
React Native requests reviewed signing
        |
native module selects signer/envelope
        |
Keychain / Keystore + biometric policy
        |
32-byte WalletUnlockKey released in native memory
        |
FresnicaSdkApi.signTransactionXdr
        |
signed XDR returned to React Native
```

`WalletUnlockKey` must never enter JavaScript.

Mnemonic / `S...` plaintext may cross the framework/native boundary only for:

- explicit initial import;
- one-time generated mnemonic backup/confirmation;
- explicit Reveal / Export after a fresh Fresnica app passcode.

Do not persist plaintext recovery material in Realm, Redux/application state, navigation state, logs, analytics or crash reports.

## Account / signer persistence

Mobile persists the conceptual graph:

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

A watch-only account has no applicable local signer. Do not persist a second wallet-type truth that can drift.

A classic account and signer may differ under Stellar signer/threshold rules. `C...` contract accounts are identities, not Ed25519 signer public keys.

## Legacy Mobile v0.1.0

`bindings/mobile` and the `mobile-sdk-v0.1.0` release remain read-only compatibility/donor material for the previous integration surface and the #81-#84 application-lifecycle migration reference.

Do not start new Mobile integration against:

- `fresnica-mobile-core`;
- `FresnicaCoreFFI.xcframework` from the legacy Mobile package;
- the legacy AAR containing React Native classes;
- `bindings/mobile/platform/**` as the authoritative system-auth implementation.

New work uses `bindings/native` outputs and `adapters/react-native`.

## Build and validation references

- Native release contract: [`native-sdk-release.md`](native-sdk-release.md)
- Framework adapter contract: [`mobile-framework-adapter-contract.md`](mobile-framework-adapter-contract.md)
- System authentication: [`mobile-system-auth.md`](mobile-system-auth.md)
- Mobile lifecycle migration: [`mobile-app-migration-pr81-pr84.md`](mobile-app-migration-pr81-pr84.md)
- React Native upgrade rules: [`react-native-upgrade-playbook.md`](react-native-upgrade-playbook.md)

The consumer-facing installation, one-time adapter build and first-app smoke-test steps are documented in [`mobile-sdk-usage.md`](mobile-sdk-usage.md).

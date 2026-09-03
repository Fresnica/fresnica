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

The latest **published** Mobile integration baseline uses these independent compatibility numbers:

```text
Native package version:       0.3.0
NATIVE_BINDING_API_VERSION:   3
SDK_API_VERSION:              5
CLIENT_API_VERSION:           5
RN adapter source version:    0.3.0
```

The 0.3.0 release adds the SEP-53 message-signing domain needed by Mobile dapp challenges. Mobile should pin the exact pre-1.0 `native-sdk-v0.3.0` release plus the matching adapter manifest rather than consuming moving development source. A package-version update is not automatically an API break; the API constants are the machine-readable compatibility boundary.

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
- `deriveMnemonicSigner`
- `reprotect`
- `deriveUnlockKey`
- `validateUnlockKey`
- `signTransactionXdr`
- `signMessage`
- `signMessageWithPasscode`
- `reveal`
- `prepareEd25519Signing`
- `prepareMessageSigning`
- `verifyMessageSignature`
- `applyEd25519Signature`

`deriveUnlockKey`, `validateUnlockKey`, raw routine `signTransactionXdr`, raw `signMessage`, `prepareMessageSigning`, and `verifyMessageSignature` are **native-only** primitives. The React Native adapter must not forward unlock-key material or those low-level message primitives to JavaScript. The high-level passcode bridge calls native `signMessageWithPasscode` directly so the derived unlock key stays inside Rust.

## React Native surface

The canonical JavaScript module remains `FresnicaCore`. It exposes the reviewed high-level surface:

- `parseAccount`
- `protectSecret`
- `protectMnemonic`
- `generateMnemonic`
- `deriveMnemonicSigner`
- `reprotect`
- `reveal`
- `prepareEd25519Signing`
- `applyEd25519Signature`
- `canUseSystemAuth`
- `hasSystemAuthDomain`
- `initializeSystemAuth`
- `registerSignerSystemAuth`
- `hasSignerSystemAuth`
- `removeSignerSystemAuth`
- `removeSystemAuthDomain`
- `signMessageWithSystemAuth`
- `signMessageWithPasscode`
- `signWithSystemAuth`
- `signWithPasscode`

The adapter performs only argument/result conversion, React Native registration/lifecycle work and the platform UI steps needed to drive SDK-owned native authorization.

The v0.2 security boundary adds two important high-level semantics:

- `deriveMnemonicSigner` derives another explicit HD index from an existing mnemonic-backed protected source without returning the mnemonic to JavaScript; the normal first index is `0`.
- system auth is one device/app-level protection domain. `initializeSystemAuth` performs the one-time system-auth prompt; later `registerSignerSystemAuth` calls verify the Fresnica passcode and wrap each new signer unlock key with the existing domain public key without another biometric prompt. Face ID/fingerprint authorizes routine signing only and never substitutes for the Fresnica passcode.

The 0.3.0 adapter adds a third high-level signing surface for dapps: `signMessageWithSystemAuth` / `signMessageWithPasscode`. They accept a framework `String`, encode it as exact UTF-8 without normalization, and invoke Native/Core SEP-53 signing. They do not expose `WalletUnlockKey`, a generic hash signer, `prepareMessageSigning`, or `verifyMessageSignature` to JavaScript. Origin, selected account/network, nonce, expiry, replay protection and challenge-size/display policy remain Mobile dapp/session responsibilities.

It must not reimplement derivation, protected-envelope parsing, signer identity checks, transaction hashing/signing, signature verification, Keychain/Keystore policy, or `WalletUnlockKey` handling.

## Secret boundary

Routine software signing is:

```text
React Native requests reviewed signing
        |
native module selects signer/envelope
        |
device System Auth Protection Domain
        |
auth-bound private unwrap after biometric/system authorization
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

RecoverySourceRecord / grouping metadata (Mobile-owned)
  shared mnemonic backup/HD grouping when applicable
```

**Account != Signer != Recovery Source.** A watch-only account has no applicable local signer. Do not persist a second wallet-type truth that can drift.

A classic account and signer may differ under Stellar signer/threshold rules. `C...` contract accounts are identities, not Ed25519 signer public keys.

## Legacy Mobile v0.1.0

The `bindings/mobile` source has been retired from `main`. The `mobile-sdk-v0.1.0` tag/release and archived documentation remain the historical compatibility record for the previous integration surface; #81-#84 application-lifecycle semantics remain migration acceptance criteria, not active source.

Do not start new Mobile integration against:

- `fresnica-mobile-core`;
- `FresnicaCoreFFI.xcframework` from the legacy Mobile package;
- the legacy AAR containing React Native classes;
- historical `bindings/mobile/platform/**` code from the v0.1.0 tag as a current authoritative system-auth implementation.

New work uses `bindings/native` outputs and `adapters/react-native`.

## Build and validation references

- Native release contract: [Native SDK release contract](../../sdk/native-release.md)
- Framework adapter contract: [Framework adapter contract](framework-adapter.md)
- System authentication: [System authentication](system-auth.md)
- Mobile lifecycle migration: [Mobile lifecycle migration](app-migration-pr81-pr84.md)
- React Native upgrade rules: [React Native upgrade rules](react-native-upgrade-playbook.md)

The consumer-facing installation, one-time adapter build and first-app smoke-test steps are documented in [Mobile SDK usage guide](sdk-usage.md).

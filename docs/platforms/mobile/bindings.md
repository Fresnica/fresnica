# Mobile / Native Binding Architecture

Status: **published Native SDK is the authoritative Mobile security surface; a separate Application Binding is the target application-capability surface**.

This document defines how Fresnica security semantics are consumed by the independent React Native Mobile application today and how that boundary relates to the target shared Application Client. The old `fresnica-mobile-core` / `mobile-sdk-v0.1.0` line is frozen as a compatibility and migration reference.

## Two distinct native boundaries

The currently published Native SDK remains security-focused:

```text
React Native application
        |
canonical Fresnica RN security adapter binary
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

The target application-capability path is separate:

```text
Mobile Feature / Flow
        |
Application Binding / FFI          (target; not yet published)
        |
Fresnica Application Client
   |                     |
   v                     v
SDK / Core          DataProvider + Repository ports
```

The application does **not** compile Rust Core, `fresnica-sdk`, UniFFI, or adapter source during normal published-Native-SDK builds.

Do not add Account/Balance/Horizon/RPC/application-persistence APIs to `FresnicaSdkApi` merely because that FFI already exists. Security ABI and Application ABI have different authority and should version independently even if a future distribution ships both.

## Products and ownership

Fresnica currently publishes/owns the security package:

- `fresnica-native-sdk-VERSION.aar` for Android;
- `FresnicaSDK-VERSION-apple.zip` containing `FresnicaSDK.xcframework` and `FresnicaSDKFFI.xcframework`;
- the stable `FresnicaSdkApi` Swift/Kotlin security surface;
- native Keychain/Keystore signer-authorization helpers;
- canonical React Native security adapter source under `adapters/react-native`;
- adapter build/compatibility tooling;
- Core/SDK/security semantics and conformance tests.

The independent Mobile application owns:

- React Native version and application toolchain;
- one-time compilation of the canonical security adapter against that exact environment;
- checked-in/controlled adapter binaries and compatibility manifest;
- Realm/native persistence **mechanisms** and migrations;
- screens, navigation, product/session policy and platform lifecycle;
- platform adapters needed by the target shared Application Client;
- temporary duplicated networking/application implementation only until an equivalent shared Client capability/Application Binding is available.

The target shared Application Client owns reusable first-party native Account/Payment/Trustline/SDEX/network/application semantics. Horizon/RPC choice belongs below the Client DataProvider boundary, not in the Native SDK security ABI.

## Version contract

The latest **published security integration** baseline uses these independent compatibility numbers:

```text
Native package version:       0.3.0
NATIVE_BINDING_API_VERSION:   3
SDK_API_VERSION:              5
CLIENT_API_VERSION:           5
RN adapter source version:    0.3.0
```

`CLIENT_API_VERSION` here is the historical Core/SDK client-security contract used by the Native SDK package. It must not be confused with the future Application Binding version for `fresnica-client` capabilities.

The 0.3.0 release adds the SEP-53 message-signing domain needed by Mobile dapp challenges. Mobile should pin the exact pre-1.0 `native-sdk-v0.3.0` release plus the matching adapter manifest rather than consuming moving development source. A package-version update is not automatically an API break; the API constants are the machine-readable compatibility boundary.

## Native security API

Kotlin package: `com.fresnica.sdk`

Swift module: `FresnicaSDK`

Primary object: `FresnicaSdkApi`

The native API exposes wallet/signer security lifecycle and signing primitives backed by the platform-neutral SDK, including:

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

`deriveUnlockKey`, `validateUnlockKey`, raw routine `signTransactionXdr`, raw `signMessage`, `prepareMessageSigning`, and `verifyMessageSignature` are **native-only** primitives. The React Native security adapter must not forward unlock-key material or those low-level message primitives to JavaScript. The high-level passcode bridge calls native `signMessageWithPasscode` directly so the derived unlock key stays inside Rust.

## React Native security surface

The canonical JavaScript module remains `FresnicaCore`. It exposes the reviewed high-level security surface:

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

## Future Application Binding

The future Application Binding should expose only mature `fresnica-client` capabilities through typed provider-neutral DTOs. It is not a second wrapper around Horizon JSON.

Before that binding is introduced, the Rust Client must first remove Terminal-specific construction assumptions:

1. wallet/contact/pending/cache persistence ownership must be explicit instead of deriving unrelated stores through `WalletStorage.home()`;
2. Classic synchronous calls and RPC/Soroban asynchronous calls need an explicit runtime boundary rather than internal `block_on`;
3. provider-specific data must terminate below public Client DTOs;
4. endpoint overrides must remain separate from the selected Stellar network/passphrase.

Account and Balance already demonstrate the desired DTO direction. History/Activity remains deferred until a stable index/history contract exists.

Application Binding versioning must be independent from `NATIVE_BINDING_API_VERSION` and the current Native security package's `CLIENT_API_VERSION` naming.

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

Mobile currently persists the conceptual graph:

```text
AccountRecord
  identity/address/network/product metadata

SignerRecord
  signer public identity
  signer kind/provider metadata
  opaque protected envelope when applicable

AccountSignerReference
  account <-> signer relationship

RecoverySourceRecord / grouping metadata
  shared mnemonic backup/HD grouping when applicable
```

**Account != Signer != Recovery Source.** A watch-only account has no applicable local signer. Do not persist a second wallet-type truth that can drift.

A classic account and signer may differ under Stellar signer/threshold rules. `C...` contract accounts are identities, not Ed25519 signer public keys.

This conceptual persistence model is useful input to the future Client Repository port, but Realm itself is a Mobile adapter choice, not the cross-platform repository contract.

## Legacy Mobile v0.1.0

The `bindings/mobile` source has been retired from `main`. The `mobile-sdk-v0.1.0` tag/release and archived documentation remain the historical compatibility record for the previous integration surface; #81-#84 application-lifecycle semantics remain migration acceptance criteria, not active source.

Do not start new Mobile security integration against:

- `fresnica-mobile-core`;
- `FresnicaCoreFFI.xcframework` from the legacy Mobile package;
- the legacy AAR containing React Native classes;
- historical `bindings/mobile/platform/**` code from the v0.1.0 tag as a current authoritative system-auth implementation.

New **security** work uses `bindings/native` outputs and `adapters/react-native`. Future shared application-capability integration uses the separately versioned Application Binding once that boundary is ready.

## Build and validation references

- Shared Application Client decision: [Shared Application Client Boundary](../../decisions/shared-application-client.md)
- Native release contract: [Native SDK release contract](../../sdk/native-release.md)
- Framework security-adapter contract: [Framework adapter contract](framework-adapter.md)
- System authentication: [System authentication](system-auth.md)
- Mobile lifecycle migration: [Mobile lifecycle migration](app-migration-pr81-pr84.md)
- React Native upgrade rules: [React Native upgrade rules](react-native-upgrade-playbook.md)

The consumer-facing security-package installation, one-time adapter build and first-app smoke-test steps are documented in [Mobile SDK usage guide](sdk-usage.md).

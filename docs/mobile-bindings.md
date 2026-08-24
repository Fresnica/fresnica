# Mobile Rust Binding Architecture

Status: **accepted pre-release integration direction**.

This document records how Fresnica Core is exposed to the Xaman-derived mobile application after the account/signer and `CoreClientApi` boundaries were stabilized.

## Decision

Use stable **UniFFI 0.32.x** to generate Swift and Kotlin bindings from `bindings/mobile`.

Do not bind Rust directly to React Native JSI/TurboModule.
Do not use the experimental UniFFI Kotlin-JNI backend yet.
Do not hand-maintain parallel Swift and JNI mappings of the Core API.

The mobile stack is:

```text
React Native UI / product state
        |
thin platform native module
        |
Swift                 Kotlin
        \             /
         stable UniFFI
              |
    fresnica-mobile-core
              |
        CoreClientApi
              |
         Fresnica Core
```

## Why this fits Xaman

The current Xaman application is React Native, but its security and utility integrations still use conventional native modules on both platforms. Examples include:

- iOS `ios/Xaman/Libs/Security/Crypto/Crypto.m`
- iOS `ios/Xaman/Libs/Security/Vault/VaultManager.m`
- iOS `ios/Xaman/Libs/Security/Authentication/Biometric/BiometricModule.m`
- Android `android/app/src/main/java/libs/security/crypto/CryptoModule.java`
- Android `android/app/src/main/java/libs/security/vault/VaultManagerModule.java`
- Android `android/app/src/main/java/libs/security/authentication/Biometric/BiometricModule.java`

Fresnica can therefore preserve useful navigation, lifecycle, biometric, Keychain/Keystore, and React Native infrastructure while replacing the secret/signing authority behind those modules.

The new path should be thin:

```text
Xaman-derived RN action
       |
FresnicaCoreModule.swift / FresnicaCoreModule.kt
       |
generated FresnicaCore Swift/Kotlin API
       |
Rust
```

The native module may translate React Native values and errors, but it MUST NOT reimplement derivation, encryption, signer identity checks, transaction hashing, or signature validation.

## Why not direct JSI

Direct Rust-to-JSI coupling was rejected for this phase because:

- Fresnica's cryptographic operations are low-frequency control operations, not a high-throughput rendering/data path;
- routine signing should keep `WalletUnlockKey` and signer orchestration in native code rather than make the JavaScript runtime the security boundary;
- the inherited Xaman app already has mature native-module infrastructure;
- JSI/TurboModule would couple the Core integration to React Native architecture changes without improving Core correctness.

A future performance-driven JSI adapter could still sit above the same `fresnica-mobile-core` API if a measured need appears.

## Why UniFFI

UniFFI removes two classes of duplicate implementation:

1. manual Rust FFI memory/error/type handling;
2. separate hand-maintained Swift and Kotlin representations of the Core facade.

The generated layer is allowed to own only language marshalling. Fresnica-owned semantics remain in Rust.

Proc macros are used instead of a duplicate UDL interface so the Rust method signatures and generated metadata cannot drift independently.

## Stable backend choice

For Android, use UniFFI's established Kotlin/JNA path first.

UniFFI 0.32 also contains an experimental Kotlin-JNI generator. Fresnica will not adopt that backend until:

- the standard Kotlin binding is integrated and measured;
- the JNI backend is no longer experimental or there is a demonstrated performance/packaging reason to accept its instability;
- the generated API remains equivalent under Fresnica's conformance tests.

The Core operations involved here are normally import, unlock-key derivation, signing, re-protection, account parsing, and explicit reveal/export. JNA call overhead is not the dominant cost of these operations.

## Threading

UniFFI objects must be safe for concurrent foreign-language access.

`MobileCoreApi` is therefore intentionally stateless. It does not retain an unlocked signer, plaintext material, `WalletUnlockKey`, or mutable `CoreClientApi` session. Each call creates a short-lived Core facade and returns owned output.

Platform code remains responsible for serializing UI flows where product semantics require it, for example passcode rotation and account persistence transactions.

## Secret boundary

Normal software signing should become:

```text
React Native requests sign
        |
native module selects account + signer
        |
Keychain / Keystore + biometric policy
        |
32-byte WalletUnlockKey released in native memory
        |
UniFFI sign_transaction_xdr
        |
Rust opens opaque signer envelope, verifies signer identity, signs
        |
signed XDR returned
```

The `WalletUnlockKey` MUST NOT be returned to JavaScript.

Mnemonic / `S...` plaintext may cross the React Native/native boundary only where the existing security contract explicitly allows it:

- initial user import when unavoidable;
- one-time generated mnemonic presentation/confirmation;
- explicit Reveal / Export after fresh app-passcode entry.

Those values must not be stored in Realm, navigation state, logs, analytics, crash reports, or ordinary Redux/state persistence.

## Account and signer persistence

The mobile persistence model remains independent from the binding technology.

```text
AccountRecord
  identity: G... / C...
  metadata

SignerRecord
  signer_public_key
  kind
  opaque protected envelope / external-signer metadata
```

A watch-only account has no local signer record.
A full software wallet has a protected-software signer record.
A Stellar additional/multisig signer may have a signer public key different from the account address.

The network/client layer determines whether a local signer is currently authorized by the account's on-chain signer/threshold state.

## Platform work after generated bindings

### iOS

1. Build Rust as static libraries for supported Apple architectures.
2. Generate Swift bindings and headers/module map.
3. Package the Rust library + generated FFI module as an XCFramework or equivalent reproducible Xcode dependency.
4. Add `FresnicaCoreModule.swift` as the React Native-facing adapter.
5. Adapt retained Keychain/biometric infrastructure to store/release per-signer `WalletUnlockKey` values.

### Android

1. Cross-compile Rust shared libraries for supported Android ABIs.
2. Generate Kotlin bindings in package `com.fresnica.core`.
3. Package native `.so` files and generated Kotlin code into the Android application/library build.
4. Add `FresnicaCoreModule.kt` as the React Native-facing adapter.
5. Adapt retained Keystore/StrongBox/biometric infrastructure to protect/release per-signer `WalletUnlockKey` values.

## CI expectations

Before platform UI integration, CI should prove:

- Rust Core tests pass;
- `fresnica-mobile-core` tests pass;
- Rust FFI library builds;
- Kotlin bindings generate from the compiled library;
- Swift bindings generate from the same compiled metadata;
- generated APIs contain the expected Core object, account/signer records, stable error surface, and external signing operations.

Later platform packaging PRs should add Android ABI builds and Apple static/XCFramework builds rather than weakening this host-level conformance gate.

# Fresnica Native SDK Binding

`fresnica-native-sdk` is the framework-neutral UniFFI layer used to build compiled native Fresnica SDK packages.

```text
fresnica-core
    |
fresnica-sdk                 shared semantic contract
    |
fresnica-native-sdk          UniFFI/native DTO glue
    |
    +-- Android SDK package
    +-- Apple SDK package (iOS + macOS Swift)
    +-- future Windows/Linux native packages
```

This is the authoritative native binding surface for new Mobile/native consumers. The transitional `bindings/mobile` facade has been retired from `main`; history remains in Git/tagged releases.

## Rules

- `NATIVE_BINDING_API_VERSION` versions the native FFI surface independently from `SDK_API_VERSION` and the Core client API.
- Account, signer, and recovery-source semantics remain separate; recovery-source grouping stays application-owned.
- Protected envelopes remain opaque.
- Routine `WalletUnlockKey` material is native-only and must not be surfaced by framework adapters to JavaScript/Dart.
- Reveal/Export requires a fresh application passcode.
- External/hardware signing uses domain-specific prepare/apply or prepare/verify operations; callbacks do not cross FFI.
- SEP-53 message signing is exposed as exact byte arrays at the native boundary; platform/framework adapters decide how a product string becomes bytes.
- The Rust crate contains no React Native, Flutter, persistence, network, Keychain/Keystore, or UI behavior.
- Platform packages may ship native signer-authorization helpers beside the generated SDK API. Those helpers protect/release `WalletUnlockKey` only inside native platform code and do not move OS authentication into Rust Core.
- Secret-bearing generated/reveal records intentionally do not implement Rust `Debug`.

## Generated API

Kotlin package: `com.fresnica.sdk`

Swift module: `FresnicaSDK`

The exported object is `FresnicaSdkApi`, exposing the reviewed Rust SDK semantic operations, including `deriveMnemonicSigner` for deriving another explicit HD index from an authenticated mnemonic-backed protected source and SEP-53 `signMessage` / `prepareMessageSigning` / `verifyMessageSignature` operations.

Run:

```sh
cargo test --manifest-path bindings/native/Cargo.toml
cargo build --manifest-path bindings/native/Cargo.toml
```

Generated language bindings are release/build outputs, not hand-maintained source.

On macOS, run `bash bindings/native/scripts/validate-apple-local.sh` to validate the compiled Apple direct-consumer package end to end. The command covers iOS and macOS Swift slices; the expanded validation passed on a real Xcode toolchain on 2026-08-25.

## Android raw-AAR dependencies

The current GitHub release shape distributes a raw AAR. Direct file consumers must also declare Kotlin stdlib 1.9.24, JNA 5.12.1 (`@aar`) and AndroidX annotation 1.8.2. These versions are part of the release manifest contract. A later Maven publication can express the same dependencies transitively; the Native SDK should not become a fat AAR.

The standalone smoke consumer under `tests/android-consumer` is compiled by `scripts/validate-android-consumer.sh` in Native SDK packaging/release CI to prove that this raw-AAR dependency contract is sufficient.

## Platform signer authorization

The generalized native packages now own the reusable platform security helpers that previously lived only in the transitional Mobile package:

- Android: `com.fresnica.sdk.security.WalletUnlockKeyStore` and `FresnicaSignerAuthorization`;
- Apple: `FresnicaWalletUnlockKeyStore` and `FresnicaSignerAuthorization`.

These are not framework adapters. They implement the platform-side credential release/signing boundary while the framework adapter only drives platform UI/lifecycle glue.

The Native SDK uses one device/app-level **System Auth Protection Domain** rather than one biometric enrollment per signer. Domain initialization performs one authenticated private-key challenge. Later signer registration derives a verified per-signer `WalletUnlockKey` from the Fresnica passcode and wraps it with the already-created domain public key, so adding a signer does not trigger another biometric prompt. Routine signing authenticates the domain private-key unwrap. Android drives the exact `Cipher` through `BiometricPrompt`; Apple uses the auth-bound `SecKey`/`LAContext` path. System auth remains lower privilege than the Fresnica passcode and cannot authorize Reveal / Export.

## Apple compiled module

Apple packaging now has a separate direct-consumer module step: `build-apple.sh` first creates `FresnicaSDKFFI.xcframework`, then archives the generated Swift API plus the native Keychain/LocalAuthentication helpers as `FresnicaSDK.xcframework`. Framework adapters should compile against `FresnicaSDK`; they must not absorb the generated SDK Swift/security sources into adapter ownership.

## macOS Swift consumer

The Apple package reuses the same `FresnicaSDK` module and semantic API on macOS. `FresnicaSDKFFI.xcframework` includes a universal macOS Rust static-library slice, and `FresnicaSDK.xcframework` includes the corresponding compiled Swift framework. The initial macOS deployment baseline is 12.0.

The native Keychain helper uses the Data Protection Keychain on macOS (`kSecUseDataProtectionKeychain`) so its `ThisDeviceOnly` access-control policy remains meaningful; routine `WalletUnlockKey` bytes still remain native-only.

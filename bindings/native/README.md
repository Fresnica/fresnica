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

It is intentionally separate from `bindings/mobile`, which preserves the transitional Mobile v0.1.0 API. New native consumers should target this generalized binding surface.

## Rules

- `NATIVE_BINDING_API_VERSION` versions the native FFI surface independently from `SDK_API_VERSION` and the Core client API.
- Account and signer identity remain separate.
- Protected envelopes remain opaque.
- Routine `WalletUnlockKey` material is native-only and must not be surfaced by framework adapters to JavaScript/Dart.
- Reveal/Export requires a fresh application passcode.
- External/hardware signing uses prepare/apply operations; callbacks do not cross FFI.
- The Rust crate contains no React Native, Flutter, persistence, network, Keychain/Keystore, or UI behavior.
- Platform packages may ship native signer-authorization helpers beside the generated SDK API. Those helpers protect/release `WalletUnlockKey` only inside native platform code and do not move OS authentication into Rust Core.
- Secret-bearing generated/reveal records intentionally do not implement Rust `Debug`.

## Generated API

Kotlin package: `com.fresnica.sdk`

Swift module: `FresnicaSDK`

The exported object is `FresnicaSdkApi`, exposing the same eleven semantic operations as the Rust SDK contract.

Run:

```sh
cargo test --manifest-path bindings/native/Cargo.toml
cargo build --manifest-path bindings/native/Cargo.toml
```

Generated language bindings are release/build outputs, not hand-maintained source.

On macOS, run `bash bindings/native/scripts/validate-apple-local.sh` to validate the compiled Apple direct-consumer package end to end. The same command covers the already-proven iOS package plus the macOS Swift slice; macOS remains pending until that expanded validation passes on a real Xcode toolchain.

## Android raw-AAR dependencies

The current GitHub release shape distributes a raw AAR. Direct file consumers must also declare Kotlin stdlib 1.9.24, JNA 5.12.1 (`@aar`) and AndroidX annotation 1.8.2. These versions are part of the release manifest contract. A later Maven publication can express the same dependencies transitively; the Native SDK should not become a fat AAR.

The standalone smoke consumer under `tests/android-consumer` is compiled by `scripts/validate-android-consumer.sh` in Native SDK packaging/release CI to prove that this raw-AAR dependency contract is sufficient.

## Platform signer authorization

The generalized native packages now own the reusable platform security helpers that previously lived only in the transitional Mobile package:

- Android: `com.fresnica.sdk.security.WalletUnlockKeyStore` and `FresnicaSignerAuthorization`;
- Apple: `FresnicaWalletUnlockKeyStore` and `FresnicaSignerAuthorization`.

These are not framework adapters. They implement the platform-side credential release/signing boundary while the framework adapter only drives platform UI/lifecycle glue. Android biometric UI still authenticates the exact `Cipher` returned by the authorization helper; Apple Keychain access performs the LocalAuthentication-gated release.

## Apple compiled module

Apple packaging now has a separate direct-consumer module step: `build-apple.sh` first creates `FresnicaSDKFFI.xcframework`, then archives the generated Swift API plus the native Keychain/LocalAuthentication helpers as `FresnicaSDK.xcframework`. Framework adapters should compile against `FresnicaSDK`; they must not absorb the generated SDK Swift/security sources into adapter ownership.

## macOS Swift consumer

The Apple package reuses the same `FresnicaSDK` module and semantic API on macOS. `FresnicaSDKFFI.xcframework` includes a universal macOS Rust static-library slice, and `FresnicaSDK.xcframework` includes the corresponding compiled Swift framework. The initial macOS deployment baseline is 12.0.

The native Keychain helper uses the Data Protection Keychain on macOS (`kSecUseDataProtectionKeychain`) so its `ThisDeviceOnly` access-control policy remains meaningful; routine `WalletUnlockKey` bytes still remain native-only.

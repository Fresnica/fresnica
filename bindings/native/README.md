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
    +-- Apple SDK package
    +-- future desktop native packages
```

It is intentionally separate from `bindings/mobile`, which preserves the transitional Mobile v0.1.0 API. New native consumers should target this generalized binding surface.

## Rules

- `NATIVE_BINDING_API_VERSION` versions the native FFI surface independently from `SDK_API_VERSION` and the Core client API.
- Account and signer identity remain separate.
- Protected envelopes remain opaque.
- Routine `WalletUnlockKey` material is native-only and must not be surfaced by framework adapters to JavaScript/Dart.
- Reveal/Export requires a fresh application passcode.
- External/hardware signing uses prepare/apply operations; callbacks do not cross FFI.
- This crate contains no React Native, Flutter, persistence, network, Keychain/Keystore, or UI behavior.
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

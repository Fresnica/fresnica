# Fresnica Universal SDK

`fresnica-sdk` is the platform-neutral semantic SDK above `fresnica-core::CoreClientApi`.

It is the common contract for native platform SDK packaging, future WASM bindings, and framework adapters. It does not depend on UniFFI, React Native, Flutter, operating-system secure storage, networking, persistence, or UI code.

```text
fresnica-core
    |
    v
fresnica-sdk                  <- stable semantic contract
    |
    +-- native binding/package layer
    |     +-- Android
    |     +-- Apple
    |     +-- Windows/Linux/macOS
    |
    +-- future WASM binding layer
```

## Contract

`FresnicaSdk` currently exposes the established Core operation families:

1. account identity parsing;
2. secret and mnemonic protection;
3. mnemonic generation;
4. passcode re-protection;
5. verified `WalletUnlockKey` derivation and validation;
6. protected software-signer transaction signing with either a verified native `WalletUnlockKey` or the composite fresh-passcode path;
7. explicit Reveal / Export;
8. external Ed25519 prepare/apply signing.

The SDK deliberately preserves these invariants:

- Account identity and signer capability are separate.
- `C...` account identity is not an Ed25519 public key.
- Watch-only accounts require no passcode, mnemonic, secret, unlock key, or protected signer envelope.
- Protected signer envelopes are opaque serialized values to consumers.
- `WalletUnlockKey` is routine native authorization material and does not authorize Reveal / Export.
- Reveal / Export requires a fresh application passcode.
- External/hardware signing uses the transport-neutral prepare/apply boundary.
- Cryptographic behavior and identity verification remain authoritative in `fresnica-core`.

## Stable boundary types

The SDK boundary uses:

- fixed-width integer fields (`u32`, `u64`) rather than platform-sized `usize`;
- byte arrays for transaction XDR, hashes, signatures, and unlock keys;
- strings for addresses and network passphrases;
- opaque JSON strings for protected signer envelopes;
- stable serialized error categories.

`SDK_API_VERSION` versions this SDK contract independently from the underlying `CLIENT_API_VERSION`.

## Security scope

The SDK does not own:

- Keychain / Keystore / DPAPI / Secret Service storage;
- biometric or system-authentication policy;
- account/signer persistence;
- ledger authorization and signer-weight resolution;
- Horizon/network state;
- product session policy or UI.

Native packaging layers may orchestrate platform authorization around SDK operations without duplicating Core cryptography.

Framework adapters must not expose `WalletUnlockKey` or raw routine signing primitives to JavaScript/Dart merely for convenience.

For browser/WASM software signing, SDK API v2 adds `sign_transaction_xdr_with_passcode(...)`: Core derives and verifies the unlock key and signs within one Rust call, so the raw `WalletUnlockKey` never crosses into JavaScript. Native clients may continue using the explicit unlock-key path behind reviewed platform secure storage/system authentication.

Browser key protection and authorization remain a separate security model rather than a copy of the native `WalletUnlockKey` model.

## Validation

Run:

```sh
cargo fmt --manifest-path sdk/rust/Cargo.toml -- --check
cargo test --manifest-path core/rust/Cargo.toml
cargo test --manifest-path sdk/rust/Cargo.toml
```

The SDK tests reuse the repository transaction-signing vectors so native/mobile/future binding layers can prove conformance against the same Core behavior.

# Fresnica Mobile Core Binding Foundation

This crate is the first mobile-facing layer above `fresnica-core::CoreClientApi`.

It is intentionally **FFI-neutral**. It does not choose UniFFI, JNI, Swift C interop, JSI, or a React Native module implementation yet. Platform bindings should wrap this crate rather than `fresnica-core` directly.

## Boundary

```text
React Native / mobile application
        |
Swift / Kotlin native module
        |
future binding generator / ABI adapter
        |
fresnica-mobile-core        <- this crate
        |
fresnica-core::CoreClientApi
        |
Core crypto / signer / transaction primitives
```

The purpose of this layer is to make the next binding choice mechanical rather than architectural.

## Rules

- Account identity and signer identity remain separate.
- Mobile receives Core protected envelopes as opaque JSON strings. It must not inspect or edit their fields.
- Binding-facing numeric values use fixed-width integers rather than Rust `usize`.
- Transaction XDR, signatures, transaction hashes, and unlock keys cross this layer as byte arrays rather than base64 text.
- Sensitive string inputs are accepted as owned values and wrapped in zeroizing storage immediately after entry.
- `WalletUnlockKey` must be exactly 32 bytes and is converted back to the Core redacted key type before use.
- Reveal/export remains a distinct passcode-only declassification operation.
- External/hardware signers use `prepare_ed25519_signing` and `apply_ed25519_signature`; no Rust callback crosses the mobile boundary.
- Stable Core error categories are preserved as `MobileCoreErrorCode`.

## Surface

`MobileCoreApi` currently exposes:

1. `parse_account`
2. `protect_secret`
3. `protect_mnemonic`
4. `generate_mnemonic`
5. `reprotect`
6. `derive_unlock_key`
7. `validate_unlock_key`
8. `sign_transaction_xdr`
9. `reveal`
10. `prepare_ed25519_signing`
11. `apply_ed25519_signature`

`MOBILE_BINDING_API_VERSION` versions this boundary separately from `CLIENT_API_VERSION` in Rust Core.

## What this crate does not own

It does not own:

- Keychain / Android Keystore persistence;
- Face ID / Touch ID / Android biometric prompts;
- Realm schemas;
- account-to-signer ledger authorization;
- hardware transport invocation;
- network or Horizon access;
- React Native / JavaScript state;
- passcode UI or application session policy.

Those remain mobile/platform responsibilities as defined in `docs/mobile-core-contract.md`.

## Validation

The crate tests every operation family against the same Core semantics used by the Python reference, including the shared transaction-signing vector under `spec/test-vectors/transaction-signing-v1.json`.

Run:

```sh
cargo test --manifest-path bindings/mobile/Cargo.toml
```

A dedicated GitHub Actions workflow runs this crate together with the Rust Core tests when either layer changes.

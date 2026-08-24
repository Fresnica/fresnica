# Fresnica Mobile Core

`fresnica-mobile-core` is the mobile-facing layer above `fresnica-core::CoreClientApi`.

The domain/API surface remains platform-neutral. Swift and Kotlin bindings are generated from this same Rust facade with **UniFFI 0.32.x**. Native React Native modules then wrap the generated Swift/Kotlin API.

Fresnica deliberately does **not** bind Rust directly to React Native JSI/TurboModule and does not use UniFFI's experimental Kotlin-JNI backend. Xaman's retained native infrastructure is based on conventional Objective-C/Java React Native modules, and Fresnica can replace the crypto/vault authority without coupling Core to the JavaScript runtime.

## Boundary

```text
React Native / Fresnica mobile UI
        |
thin React Native native module
        |
Swift / Kotlin
        |
stable UniFFI-generated bindings
        |
fresnica-mobile-core        <- this crate
        |
fresnica-core::CoreClientApi
        |
Core crypto / signer / transaction primitives
```

The Swift/Kotlin generation choice is implementation glue. Account, signer, protection, and signing semantics remain defined below it.

## Rules

- Account identity and signer identity remain separate.
- Mobile receives Core protected envelopes as opaque JSON strings. It must not inspect or edit their fields.
- Binding-facing numeric values use fixed-width integers rather than Rust `usize`.
- Transaction XDR, signatures, transaction hashes, and unlock keys cross this layer as byte arrays rather than base64 text.
- Sensitive string inputs are accepted as owned values and wrapped in zeroizing Rust storage immediately after entry.
- `WalletUnlockKey` must be exactly 32 bytes and is converted back to the Core redacted key type before use.
- Reveal/export remains a distinct passcode-only declassification operation.
- External/hardware signers use `prepare_ed25519_signing` and `apply_ed25519_signature`; no Rust callback crosses the mobile boundary.
- Stable Core error categories are preserved by `MobileCoreError` / `MobileCoreErrorCode`.
- `MobileCoreApi` is stateless. Each call creates a short-lived `CoreClientApi`; no unlocked signer or secret-bearing Core session is retained between FFI calls.

## Surface

`MobileCoreApi` exposes:

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

## UniFFI

The crate uses proc-macro definitions directly in Rust rather than duplicating the API in a UDL file.

The crate builds all forms needed by the next packaging step:

```toml
crate-type = ["lib", "cdylib", "staticlib"]
```

- `cdylib` is used for host/library-mode generation and Android shared libraries.
- `staticlib` is required for the future iOS XCFramework packaging path.
- normal `lib` output keeps Rust tests and direct Rust consumers straightforward.

Generate host bindings after building the library:

```sh
cargo build --manifest-path bindings/mobile/Cargo.toml

cargo run --manifest-path bindings/mobile/Cargo.toml \
  --features bindgen --bin uniffi-bindgen -- \
  generate --library bindings/mobile/target/debug/libfresnica_mobile_core.so \
  --language kotlin --out-dir /tmp/fresnica-kotlin

cargo run --manifest-path bindings/mobile/Cargo.toml \
  --features bindgen --bin uniffi-bindgen -- \
  generate --library bindings/mobile/target/debug/libfresnica_mobile_core.so \
  --language swift --out-dir /tmp/fresnica-swift
```

Use the platform library extension on macOS/iOS builds.

Configuration lives in `uniffi.toml`:

- Kotlin package: `com.fresnica.core`
- Android mode enabled
- Kotlin records generated immutable
- Swift module: `FresnicaCore`

## Sensitive generated records

UniFFI records are ordinary Swift/Kotlin value types. Some returned records intentionally contain plaintext only during explicit import/generation/reveal flows, for example `MobileGeneratedMnemonic` and `MobileExportedSigningMaterial`.

Platform code MUST NOT log, stringify for diagnostics, persist, serialize into React Native navigation state, or send these records to telemetry. Kotlin data-class `toString()` in particular must not be used on secret-bearing records.

Routine signing does not return these records. It crosses the native boundary using only:

- opaque encrypted envelope;
- 32-byte `WalletUnlockKey` released by platform authorization;
- expected signer public key;
- transaction XDR / network passphrase.

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

The Rust facade tests every operation family against the same Core semantics used by the Python reference, including the shared transaction-signing vector under `spec/test-vectors/transaction-signing-v1.json`.

Run:

```sh
cargo test --manifest-path bindings/mobile/Cargo.toml
```

GitHub Actions additionally builds the FFI library and generates both Kotlin and Swift bindings from the compiled library metadata so proc-macro/API drift fails CI.

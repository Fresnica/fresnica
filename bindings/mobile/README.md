# Fresnica Mobile Core

`fresnica-mobile-core` is the transitional Mobile v0.1.0 UniFFI compatibility facade above the platform-neutral `fresnica-sdk` contract.

Swift and Kotlin bindings are generated from this Rust facade with **UniFFI 0.32.x**. The public Mobile API remains stable while wallet/signing semantics move out of the Mobile-specific layer and into `fresnica-sdk`.

Fresnica deliberately does **not** bind Rust directly to React Native JSI/TurboModule and does not use UniFFI's experimental Kotlin-JNI backend. React Native modules wrap the generated native Swift/Kotlin API instead.

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
fresnica-mobile-core        <- compatibility binding only
        |
fresnica-sdk                <- shared semantic SDK contract
        |
fresnica-core::CoreClientApi
        |
Core crypto / signer / transaction primitives
```

The Mobile facade owns UniFFI DTO/error translation only. It does not reproduce Core cryptography, signer identity checks, protected-envelope parsing, unlock-key validation, transaction hashing, or signature verification.

## Compatibility rules

- `MOBILE_BINDING_API_VERSION` remains `2` for the Mobile v0.1.0 surface.
- Existing Swift/Kotlin type and method names remain unchanged.
- Account identity and signer identity remain separate.
- Protected signer envelopes remain opaque JSON strings outside SDK/Core.
- Transaction XDR, signatures, transaction hashes, and unlock keys cross this layer as byte arrays rather than base64 text.
- Secret/mnemonic/passcode zeroization and protected-envelope validation are performed below this compatibility layer by `fresnica-sdk` / Core.
- `WalletUnlockKey` remains routine native-signing material only; it does not authorize Reveal/Export.
- Reveal/export remains a distinct fresh-passcode declassification operation.
- External/hardware signers use `prepare_ed25519_signing` and `apply_ed25519_signature`; no Rust callback crosses the mobile boundary.
- Stable SDK/Core error categories are preserved by `MobileCoreError` / `MobileCoreErrorCode`.
- `MobileCoreApi` is stateless and retains no unlocked signer or secret-bearing session between FFI calls.

## Surface

`MobileCoreApi` preserves the existing eleven operations:

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

The Mobile binding version remains independent from both the universal SDK API version and the Rust Core client API version.

## UniFFI

The crate uses proc-macro definitions directly in Rust rather than duplicating the API in a UDL file.

```toml
crate-type = ["lib", "cdylib", "staticlib"]
```

- `cdylib` is used for host/library-mode generation and Android shared libraries.
- `staticlib` is used by Apple packaging.
- normal `lib` output keeps Rust tests straightforward.

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

UniFFI records are ordinary Swift/Kotlin value types. Some returned records intentionally contain plaintext only during explicit generation/reveal flows, for example `MobileGeneratedMnemonic` and `MobileExportedSigningMaterial`.

Platform code MUST NOT log, stringify for diagnostics, persist, serialize into React Native navigation state, or send these records to telemetry. Kotlin data-class `toString()` in particular must not be used on secret-bearing records.

Routine signing crosses the native boundary using only:

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

Those remain application/platform responsibilities.

## Validation

The compatibility facade re-runs the same operation families and shared transaction-signing vector used by the universal SDK and Core:

```sh
cargo test --manifest-path bindings/mobile/Cargo.toml
```

GitHub Actions additionally tests Core and `fresnica-sdk`, builds the FFI library, and generates Kotlin and Swift bindings from compiled library metadata so dependency, proc-macro, and API drift fail CI.

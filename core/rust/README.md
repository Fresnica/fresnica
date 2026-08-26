# Fresnica Core (Rust)

This directory is the production Rust Core for Fresnica.

The Python reference remains the behavioral authority while stable semantics are ported. Rust code should reuse established Stellar primitives and reproduce stable test-vector behavior instead of introducing a parallel wallet model.

## Current scope

Implemented production primitives currently include:

- Classic Stellar account identities (`G...`)
- Contract account identities (`C...`) without contract runtime assumptions
- SEP-0005 deterministic Classic public-key derivation, mnemonic generation, and supported-language detection
- Classic Ed25519 software signer
- External Ed25519 transaction signer for hardware, device, or process-backed providers
- Classic transaction envelope hashing and decorated-signature attachment through the official `stellar-xdr` crate
- Canonical password-protected wallet envelopes using Scrypt + AES-256-GCM
- protected wallet initialization from Stellar secret or mnemonic material
- protected mnemonic generation for new software wallets
- `WalletUnlockKey`, the 32-byte Scrypt output used to open that same canonical envelope
- Verified unlock-key derivation with public-key identity validation before client enrollment
- One-shot protected transaction signing using `WalletUnlockKey`
- Explicit passcode-only signing-material export for user-requested reveal/migration flows
- Agent Access capability checks before Classic transaction signing
- a versioned stdin/stdout machine bridge (`fresnica-core`) used by the Python TUI as the first real Rust Core client

Transaction building, network submission, client persistence, OS authentication, SDEX, anchors, Soroban account authorization, passkeys, and UI remain outside the current Rust Core slice.

## Client / Core boundary

Rust Core does not implement OS authentication or secure-storage APIs.

TUI/CLI, desktop, mobile, and other clients own:

- Keychain / Keystore / platform credential storage;
- Face ID, Touch ID, Windows Hello, Android biometrics, PAM, and equivalent OS policy;
- application/session authorization;
- persistence of the opaque Core wallet envelope;
- persistence/protection and invalidation of client-held unlock keys.

All software-wallet clients converge on one Core input for routine signing: `WalletUnlockKey`.

A client obtains a key for enrollment through `derive_verified_unlock_key`, which derives the Scrypt key from the app passcode and canonical envelope, opens the envelope, reconstructs the signer, and verifies the expected public key before returning the key.

No second wallet ciphertext or independent system wallet key is created.

See [`docs/core/client-security.md`](../../docs/core/client-security.md), [`docs/core/client-protocol.md`](../../docs/core/client-protocol.md), and [`docs/core/protection.md`](../../docs/core/protection.md).

## First real client: Python TUI

The Python implementation still owns product orchestration, Horizon/network access, local databases, caches, contacts, and Textual UI. When `FRESNICA_CORE_BIN` points to the `fresnica-core` binary, or that binary is available on `PATH`, software-wallet cryptographic operations are delegated to this Rust Core:

- create/import and protected-envelope construction;
- Passcode -> verified `WalletUnlockKey` derivation;
- unlock-key validation before a client session is established;
- transaction signing;
- explicit passcode-only Reveal / Export.

An unlocked Rust-backed Python wallet uses a protected-signer adapter and does not hold a Python private-key `Keypair`.

The stdin/stdout protocol is a verification transport, not a requirement for future clients. A native Rust CLI can link this crate directly; mobile or desktop clients may use UniFFI, C ABI, JNI, Swift, or another native binding while preserving the same operations and security contract.

## Signing boundary

Classic transaction signing signs an exact 32-byte Stellar transaction hash. `TransactionSigningRequest` also carries raw envelope XDR and the network passphrase so an external signer can inspect public transaction context before approving a signature; those fields are review context, not an alternate payload to sign.

External signers hold only the declared Stellar public key and a provider callback. Fresnica verifies the provider's returned Ed25519 signature against the exact transaction hash before mutating the envelope.

For local software wallets, `sign_protected_transaction_envelope` accepts the canonical protected wallet envelope plus `WalletUnlockKey`, verifies the expected public identity, signs, and drops the secret-bearing signer without returning private signing material to the client.

Arbitrary message signing remains a separate future capability following SEP-53 and must preserve SEP-53 domain separation.

## Secret-protection boundary

The canonical software-wallet format remains the version-1 Scrypt + AES-256-GCM password envelope. Each wallet has independent random KDF salt and AEAD nonce material, so one Fresnica app passcode still produces a different unlock key for every wallet.

`SystemProtectionProvider` and `SystemKeyStore` are not part of Core. OS-specific authorization is a client concern.

`WalletUnlockKey` uses zeroizing storage and redacted `Debug`. Intermediate decrypted buffers and sensitive signing-material strings also use zeroizing containers where practical.

Changing the app passcode or re-encrypting with a new salt changes the unlock key. Clients must invalidate and re-enroll any system-protected copy of the old key.

## Reveal / Export boundary

`export_signing_material` is deliberately separate from routine signing and accepts a fresh app passcode rather than `WalletUnlockKey`.

It reconstructs and validates the signer identity before returning either the stored Stellar secret or the stored mnemonic plus passphrase/derivation metadata. A client-held unlock key or system-authenticated session is insufficient to use this API.

See [`docs/core/secret-export.md`](../../docs/core/secret-export.md).

## Agent Access boundary

Agent Access authorizes use of an existing signer; it does not give an agent wallet secret material. `AgentCapability` binds a Classic G account to one network, an explicit Stellar `OperationType` allowlist, maximum operation count, total transaction-fee ceiling, and optional expiry.

The current slice is deliberately fail-closed and operation-level. Destination, asset, amount, market, contract, and argument constraints must be added before broad autonomous capabilities are exposed.

## Validation

Run the Rust Core test suite:

```sh
cargo test --manifest-path core/rust/Cargo.toml
```

Build the machine bridge / standalone executable:

```sh
cargo build --release --manifest-path core/rust/Cargo.toml --bin fresnica-core
```

CI also builds that binary and runs the Python-to-Rust integration suite against shared wallet/protection/transaction vectors.

Future slices should consume `spec/test-vectors` where a stable language-neutral contract already exists.

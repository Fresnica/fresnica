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
- SEP-53 v1.0.0 message payload/hash/sign/verify with separate software and external-message signer contracts
- Classic transaction envelope hashing and decorated-signature attachment through the official `stellar-xdr` crate
- Canonical password-protected wallet envelopes using Scrypt + AES-256-GCM
- protected wallet initialization from Stellar secret or mnemonic material
- protected mnemonic generation for new software wallets
- `WalletUnlockKey`, the 32-byte Scrypt output used to open that same canonical envelope
- Verified unlock-key derivation with public-key identity validation before client enrollment
- One-shot protected transaction and SEP-53 message signing using `WalletUnlockKey`
- Explicit passcode-only signing-material export for user-requested reveal/migration flows
- Agent Access capability checks before Classic transaction signing

Transaction building, network submission, client persistence, OS authentication, SDEX, anchors, passkey/provider mechanics, and UI remain outside the Rust Core security boundary.

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

## Process consumers

Rust Core no longer owns a process transport. RefPython and suitable non-Rust hosts use the versioned [`fresnica-process-binding`](../../bindings/process/README.md), which delegates through the platform-neutral `fresnica-sdk` semantic boundary. Rust clients may link Core/SDK directly; native clients normally consume the Native SDK.

## Signing boundary

Classic transaction signing signs an exact 32-byte Stellar transaction hash. `TransactionSigningRequest` also carries raw envelope XDR and the network passphrase so an external signer can inspect public transaction context before approving a signature; those fields are review context, not an alternate payload to sign.

External signers hold only the declared Stellar public key and a provider callback. Fresnica verifies the provider's returned Ed25519 signature against the exact transaction hash before mutating the envelope.

For local software wallets, `sign_protected_transaction_envelope` accepts the canonical protected wallet envelope plus `WalletUnlockKey`, verifies the expected public identity, signs, and drops the secret-bearing signer without returning private signing material to the client.

SEP-53 message signing is a separate public capability. Core prepends the exact UTF-8 prefix `Stellar Signed Message:\n`, appends the caller's exact message bytes without normalization, hashes the resulting payload once with SHA-256, and signs/verifies that digest. `MessageSigningRequest` carries the original message, encoded SEP-53 payload, and digest so an external provider can review the semantic object instead of receiving only an opaque hash. Fresnica verifies the provider's returned signature before accepting it.

SEP-53 itself has no network passphrase or dapp-origin/replay semantics. Those belong to the reviewed challenge/session above Core; Core does not silently inject them and does not expose a generic `sign_hash` oracle.

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

Agent Access authorizes use of an existing signer; it must never give an agent wallet secret material, wallet passphrase, Reveal capability or raw `WalletUnlockKey`.

The current `AgentCapability` is dormant prototype evidence only. It binds a Classic G account to one network, an `OperationType` allowlist, maximum operation count, fee ceiling and optional signing-time expiry, but it has no production SDK/binding/transport consumer. Operation type is not transaction authority: destination, asset, amount/value, market, contract arguments, transaction timebounds and stateful replay/budget limits are not yet constrained.

Do not expose or extend this prototype as a generic operation allowlist. A future design should separate stateful grant/token/revocation accounting from deterministic Core transaction-policy evaluation, require the exact authorized envelope to be the envelope signed, and begin with a narrow operation-specific policy plus negative tests. Process Binding is a privileged owner interface and is not the Agent Access transport.

## Validation

Run the Rust Core test suite:

```sh
cargo test --manifest-path core/rust/Cargo.toml
```

CI also builds that binary and runs the Python-to-Rust integration suite against shared wallet/protection/transaction vectors.

Future slices should consume `spec/test-vectors` where a stable language-neutral contract already exists.

# Rust Core Client Protocol

Status: **pre-release protocol v2 for the first real Core client**.

This document describes the machine-facing protocol used by the current Python TUI/CLI to exercise the production Rust Core. It defines operation semantics, not a permanent transport requirement. A Rust CLI may link the crate directly; mobile/desktop bindings may later use UniFFI, C ABI, JNI, Swift, or another native transport while preserving the same operations and security rules.

Protocol v2 incorporates the Account/Signer separation used by the Mobile/Core contract. Because Fresnica has not shipped a public wallet release, the Python reference client is updated in lockstep rather than carrying a v1 compatibility shim.

## 1. Process transport

The reference bridge binary is `fresnica-core`.

It reads exactly one JSON request from stdin and writes exactly one JSON response to stdout, then exits.

Sensitive values MUST NOT be passed as command-line arguments or environment variables. This includes:

- Fresnica app passcode;
- mnemonic;
- BIP39 passphrase;
- Stellar `S...` secret;
- `WalletUnlockKey`.

The bridge does not log requests or secret-bearing responses.

Success response:

```json
{
  "ok": true,
  "protocol_version": 2,
  "result": {}
}
```

Failure response:

```json
{
  "ok": false,
  "protocol_version": 2,
  "error": {
    "code": "invalid-passcode",
    "message": "invalid wallet passcode"
  }
}
```

## 2. Identity and naming rules

The protocol distinguishes account identity from signer identity.

- `account_address` means the `G...` or `C...` identity the user observes.
- `signer_public_key` means an Ed25519 `G...` key capable of producing a Stellar signature.
- `expected_signer_public_key` is a security assertion supplied by the client when it already knows which signer is being attached/unlocked/used.

For a simple master-key software wallet, `account_address == signer_public_key`. This equality MUST NOT be assumed by the generic signing operations because Stellar accounts can use additional/multisig signers.

A protected envelope belongs to a software signer, not to an Account record.

## 3. Operations

### `version`

Returns Core API and process protocol versions. Contains no wallet material.

### `parse-account`

Input:

- Stellar account/address string.

Core validates the StrKey and classifies it.

Output:

- validated address;
- `kind`: `classic` or `contract`;
- `public_key`: the same `G...` value for classic identities, otherwise `null`.

This is the watch-only initialization path. It requires no passcode or signer material.

### `protect-secret`

Input:

- app passcode;
- Stellar `S...` secret;
- optional `expected_signer_public_key`.

Core:

1. validates the secret;
2. derives its signer `G...` public key;
3. if an expected signer is present, verifies identity before returning anything;
4. creates the canonical password-protected software-signer envelope.

Output:

- `signer_public_key`;
- encrypted signer envelope.

Providing `expected_signer_public_key` is the ordinary watch-only-upgrade / signer-attachment path.

### `protect-mnemonic`

Input:

- app passcode;
- mnemonic;
- optional BIP39 passphrase;
- account index;
- mnemonic language;
- optional `expected_signer_public_key`.

Core validates SEP-0005 derivation, derives the signer identity, verifies the optional expected signer, and protects the original recovery material in the canonical envelope.

### `generate-mnemonic`

Input:

- app passcode;
- BIP39 language and entropy strength;
- optional BIP39 passphrase;
- account index.

Core generates the mnemonic with OS randomness, derives the Stellar signer identity, protects it, and returns:

- `signer_public_key`;
- protected envelope;
- mnemonic once;
- language and index.

### `reprotect`

Input:

- canonical software-signer envelope;
- current app passcode;
- new app passcode;
- `expected_signer_public_key`.

Core decrypts the existing envelope internally, reconstructs and verifies the signer identity, then protects the same recovery material with fresh encryption parameters under the new passcode.

Output:

- the same `signer_public_key`;
- a new protected envelope.

Plaintext recovery material is never returned. Old `WalletUnlockKey` records become stale and must be invalidated by the client.

### `derive-unlock-key`

Input:

- canonical encrypted signer envelope;
- app passcode;
- `expected_signer_public_key`.

Core derives the per-envelope 32-byte `WalletUnlockKey`, decrypts the envelope, reconstructs the signer, verifies the expected signer public key, then returns the unlock key.

This is the enrollment/passcode-fallback path.

### `validate-unlock-key`

Input:

- canonical encrypted signer envelope;
- `WalletUnlockKey`;
- `expected_signer_public_key`.

Core decrypts, reconstructs the signer, and verifies identity. No signing material is returned.

Clients MUST validate a system-auth-released key before treating the signer as unlocked.

### `sign-transaction`

Input:

- canonical encrypted signer envelope;
- `WalletUnlockKey`;
- `expected_signer_public_key`;
- base64 Stellar transaction-envelope XDR;
- exact Stellar network passphrase.

Core:

1. validates the unlock key against the envelope;
2. reconstructs the signer;
3. verifies signer identity;
4. hashes the exact transaction using official Stellar XDR semantics;
5. signs and verifies the Ed25519 signature;
6. returns signed transaction-envelope XDR.

The signer public key is not required to equal the transaction source account. Account-level signer authorization is resolved outside this local secret-envelope operation.

### `reveal`

Input:

- canonical encrypted signer envelope;
- fresh app passcode;
- `expected_signer_public_key`.

Output is the original recovery material:

- `S...` for a secret-key import; or
- mnemonic + BIP39 passphrase + index + language for a mnemonic signer.

`WalletUnlockKey` is deliberately not accepted by this operation.

### `prepare-ed25519-signing`

Input:

- base64 Stellar transaction-envelope XDR;
- exact Stellar network passphrase.

Output contains only public signing material:

- base64 32-byte transaction hash;
- base64 normalized transaction XDR;
- network passphrase.

This request can be handed to a hardware/device/external Ed25519 signer without exposing any local Core secret.

### `apply-ed25519-signature`

Input:

- base64 Stellar transaction-envelope XDR;
- exact Stellar network passphrase;
- `signer_public_key`;
- base64 64-byte Ed25519 signature.

Core recomputes the transaction hash, verifies the signature against `signer_public_key`, rejects duplicate/invalid signatures, appends the decorated signature, and returns signed transaction XDR.

The external signer never receives or returns a `WalletUnlockKey`.

## 4. Python TUI integration

The Python runtime discovers the Rust Core in this order:

1. explicit `FRESNICA_CORE_BIN` path;
2. `fresnica-core` on `PATH`;
3. otherwise the pure-Python behavioral reference remains available.

When Rust Core is active:

```text
Python TUI / services
        |
        | identity / signer envelope / credentials / transaction XDR
        v
fresnica-core process adapter
        |
        | typed calls
        v
CoreClientApi
        |
        v
Rust fresnica-core library
```

The process binary MUST remain a transport adapter. It owns JSON/base64 decoding and protocol responses, not a parallel implementation of Core cryptography or error classification.

Python still owns:

- Textual UI;
- Horizon/network access;
- account metadata and persistence;
- portfolio/history presentation;
- local database and caches;
- contacts;
- SDEX/anchor product orchestration;
- account-to-signer authorization resolution;
- OS/system-auth adapters.

Python does not need to decrypt Rust-backed software-signer material or hold a Stellar private-key object for routine signing.

## 5. Rust CLI packaging

A native CLI can bypass the process protocol and link `fresnica-core` directly.

The current bridge itself is a normal Rust binary and can be built as one executable:

```text
cargo build --release --manifest-path core/rust/Cargo.toml --bin fresnica-core
```

A future user-facing Rust CLI should reuse `CoreClientApi` directly rather than spawning `fresnica-core` recursively.

## 6. OS authentication remains outside Core

The process protocol does not add Keychain, Windows Hello, Android Keystore, LocalAuthentication, PAM, Secret Service, or any other OS API to Rust Core.

A client performs its platform-specific authorization and then submits the resulting standard `WalletUnlockKey` to `validate-unlock-key` / `sign-transaction` for protected software signers.

Reveal and re-protection remain passcode-based. Hardware/external signers follow their provider-specific authorization path and use the prepare/apply operations.

## 7. Stable error categories

Bindings and process clients should rely on stable machine-readable categories rather than Rust error strings. At minimum protocol v2 preserves/distinguishes:

- `invalid-input`;
- `invalid-passcode`;
- `invalid-unlock-key`;
- `invalid-protected-data`;
- `identity-mismatch`;
- `invalid-transaction`;
- `core-error`.

Provider-specific categories may be added later without changing the fundamental Account/Signer boundary.

## 8. Compatibility rule

Before public release this protocol may change with coordinated clients.

Protocol v2 intentionally replaces v1 naming rather than supporting ambiguous aliases such as `expected_public_key`.

After a public release, persisted signer-envelope formats require migration compatibility, and externally shipped Core-client protocols require explicit versioning/backward-compatibility policy.

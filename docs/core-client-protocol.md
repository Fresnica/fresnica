# Rust Core Client Protocol

Status: **pre-release protocol v1 for the first real Core client**.

This document describes the machine-facing protocol used by the current Python TUI/CLI to exercise the production Rust Core. It defines operation semantics, not a permanent transport requirement. A Rust CLI may link the crate directly; mobile/desktop bindings may later use UniFFI, C ABI, JNI, Swift, or another native transport while preserving the same operations and security rules.

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
  "protocol_version": 1,
  "result": {}
}
```

Failure response:

```json
{
  "ok": false,
  "protocol_version": 1,
  "error": {
    "code": "invalid-passcode",
    "message": "invalid wallet passcode"
  }
}
```

## 2. Operations

### `version`

Returns Core and protocol versions. Contains no wallet material.

### `protect-secret`

Input:

- app passcode;
- Stellar `S...` secret.

Core:

1. validates the secret;
2. derives the canonical `G...` identity;
3. creates the canonical password-protected envelope.

Output:

- public key;
- encrypted wallet envelope.

### `protect-mnemonic`

Input:

- app passcode;
- mnemonic;
- optional BIP39 passphrase;
- account index;
- mnemonic language.

Core validates SEP-0005 derivation, derives the account identity, and protects the original recovery material in the canonical envelope.

### `generate-mnemonic`

Input:

- app passcode;
- BIP39 language and entropy strength;
- optional BIP39 passphrase;
- account index.

Core generates the mnemonic with OS randomness, derives the Stellar identity, protects it, and returns the mnemonic once so the client can perform the explicit backup workflow.

### `derive-unlock-key`

Input:

- canonical encrypted envelope;
- app passcode;
- expected public key.

Core derives the per-envelope 32-byte `WalletUnlockKey`, decrypts the envelope, reconstructs the signer, verifies the expected public key, then returns the unlock key.

This is the enrollment/passcode-fallback path.

### `validate-unlock-key`

Input:

- canonical encrypted envelope;
- `WalletUnlockKey`;
- expected public key.

Core decrypts, reconstructs the signer, and verifies identity. No signing material is returned.

Clients MUST validate a system-auth-released key before treating the wallet as unlocked.

### `sign-transaction`

Input:

- canonical encrypted envelope;
- `WalletUnlockKey`;
- expected public key;
- base64 Stellar transaction-envelope XDR;
- exact Stellar network passphrase.

Core:

1. validates the unlock key against the envelope;
2. reconstructs the signer;
3. verifies wallet identity;
4. hashes the exact transaction using official Stellar XDR semantics;
5. signs and verifies the Ed25519 signature;
6. returns signed transaction-envelope XDR.

The mnemonic/private key never crosses back to the client.

### `reveal`

Input:

- canonical encrypted envelope;
- fresh app passcode;
- expected public key.

Output is the original recovery material:

- `S...` for a secret-key import; or
- mnemonic + BIP39 passphrase + index + language for a mnemonic wallet.

`WalletUnlockKey` is deliberately not accepted by this operation.

## 3. Python TUI integration

The Python runtime discovers the Rust Core in this order:

1. explicit `FRESNICA_CORE_BIN` path;
2. `fresnica-core` on `PATH`;
3. otherwise the pure-Python behavioral reference remains available.

When Rust Core is active:

```text
Python TUI / services
        |
        | envelope + passcode/unlock key + transaction XDR
        v
fresnica-core
        |
        v
Rust fresnica-core crate
```

Python still owns:

- Textual UI;
- Horizon/network access;
- portfolio/history presentation;
- local database and caches;
- contacts;
- SDEX/anchor product orchestration;
- OS/system-auth adapters.

Python no longer needs to decrypt software-wallet signing material or hold a Stellar private-key object for an unlocked Rust-backed wallet.

## 4. Rust CLI packaging

A native CLI can bypass the process protocol and link `fresnica-core` directly.

The current bridge itself is already a normal Rust binary and can be built as one executable:

```text
cargo build --release --manifest-path core/rust/Cargo.toml --bin fresnica-core
```

A future user-facing Rust CLI should reuse the crate APIs directly rather than spawning `fresnica-core` recursively.

## 5. OS authentication remains outside Core

The process protocol does not add Keychain, Windows Hello, Android Keystore, LocalAuthentication, PAM, Secret Service, or any other OS API to Rust Core.

A client performs its platform-specific authorization and then submits the resulting standard `WalletUnlockKey` to `validate-unlock-key` / `sign-transaction`.

Reveal remains passcode-only regardless of transport.

## 6. Compatibility rule

Before public release this protocol may change with coordinated clients.

After a public release, persisted wallet-envelope formats require migration compatibility, and externally shipped Core-client protocols require explicit versioning/backward-compatibility policy.

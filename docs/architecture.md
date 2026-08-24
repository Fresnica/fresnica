# Fresnica Architecture

## Current Phase

Python reference implementation plus production Rust Core slices.

The Python implementation remains a behavioral authority and source of stable test vectors while production semantics are moved into Rust Core.

## Architecture

```text
Clients / Platform Layers
Mobile / Desktop / CLI / TUI / SDK
            |
      CoreClientApi
            |
         Rust Core
            |
      Stellar Adapter
            |
      Stellar Network
```

Platform clients own UI, persistence, operating-system integration, network state, and lifecycle. Rust Core owns Stellar identity parsing, cryptography, signer behavior, transaction-signing semantics, and security-critical signer identity checks.

`CoreClientApi` is the transport-neutral library boundary. Process JSON, UniFFI, JNI, Swift, C ABI, or other bindings adapt native types to this same facade rather than reimplementing Core behavior.

## Mobile / Core boundary

For `fresnica-mobile`, the Xaman-derived platform layer may continue to own:

- iOS Keychain / Android Keystore integration;
- biometrics and system authentication;
- Realm/database encryption and persistence;
- app lock/session behavior;
- React Native UI and platform lifecycle;
- Horizon/RPC state and current account signer authorization.

Rust Core is authoritative for protected software-signer formats, key derivation, software/external signer semantics, identity verification, transaction hashes, signatures, and re-protection.

Mobile stores Core-generated encrypted signer envelopes as opaque data. System authentication authorizes signer use; it does not define a second signer encryption format.

See [Mobile / Rust Core Vault Contract](mobile-core-contract.md), [Client / Rust Core Security Contract](client-core-security.md), and [Wallet Protection Model](protection.md).

## Core Concepts

### Account Identity

An account identity is the chain object the user observes.

Current Core identity parsing recognizes:

- classic Stellar `G...` identities;
- Soroban contract `C...` identities.

Account identity contains no implication that Fresnica owns a private key for it.

A watch-only account is simply an account identity without a locally available signer.

### Signer

A signer is a capability that can produce or authorize a signature. It is not the account record.

Supported/future signer forms include:

- protected software signer;
- hardware wallet;
- mobile/platform-backed external Ed25519 signer;
- future passkey/contract authorization.

For a simple master-key software wallet:

```text
Account GABC...
Signer  GABC...
```

For Stellar additional signers or multisig:

```text
Account GABC...
Signer  G111...
Signer  G222...
```

Therefore generic signing APIs validate the **expected signer public key**, not an assumed wallet/account public key. Whether a signer is currently authorized for an account depends on ledger state and client policy and is not encoded into a local software-signer envelope.

### Protected Software Signer

A protected software signer contains:

```text
signer_public_key
opaque protected envelope
```

The envelope protects mnemonic or `S...` recovery material. It is owned semantically by the signer, not by the Account record.

This permits:

- account creation with a software signer;
- watch-only account with no signer;
- later attachment of matching signing material;
- removal of local signing material without deleting the account;
- future multiple signers per account.

### Wallet

"Wallet" is a user/product aggregate, not a cryptographic primitive that must collapse account identity and signer state into one object.

A client wallet may contain or reference:

```text
Wallet
  AccountRecord
  zero or more SignerRecords
  network / name / UI metadata
```

Core APIs therefore operate on account identities and signer capabilities directly where security semantics require it.

### Transaction Intent

Transaction flow:

```text
Intent
  |
Build Transaction
  |
Review
  |
Select authorized signer(s)
  |
Sign
  |
Submit
```

Transaction construction and ledger authorization may be client/network concerns, while cryptographic hash/signature semantics remain Core-authoritative.

### System Authentication

System authentication is signer authorization and remains independent from account identity and signer cipher format.

For protected software signers, successful platform authorization may release a per-signer `WalletUnlockKey`. For hardware/external signers, it may authorize provider invocation without any local Core private material.

### Re-protection

Changing the Fresnica app passcode re-protects software signer envelopes inside Core. Clients must not implement password rotation by exporting plaintext recovery material and encrypting it themselves.

Global passcode rotation across multiple signers is client orchestration around an atomic/recoverable batch of Core `reprotect` operations.

## Rust Core clients

- Mobile
- Desktop
- CLI
- TUI
- SDK

Clients must consume the same `CoreClientApi` identity, protection, signing, and error semantics rather than creating parallel cryptographic implementations.

The process binary is only one adapter over this library API and should contain transport concerns only.

## History Cache

History is a network-and-account-scoped cache of raw Horizon operations, not a full-chain indexer. The default cache retains the newest 2,000 operations; users may opt into keeping all history that their Horizon source still exposes. Empty caches are built from the current head backwards, while later refreshes always move from the newest local cursor forward to the current head.

See [History Cache Model](history-cache.md) and the [Architecture Decision Log](decision-log.md).

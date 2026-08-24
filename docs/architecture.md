# Fresnica Architecture

## Current Phase

Python reference implementation plus production Rust Core slices.

The Python implementation remains a behavioral authority and source of stable test vectors while production semantics are moved into Rust Core.

## Architecture

```text
Clients / Platform Layers
Mobile / Desktop / CLI / TUI / SDK
            |
         Rust Core
            |
      Stellar Adapter
            |
      Stellar Network
```

Platform clients own UI, persistence, operating-system integration, and lifecycle. Rust Core owns wallet semantics, cryptography, signer behavior, transaction-signing semantics, and policy.

## Mobile / Core boundary

For `fresnica-mobile`, the Xaman-derived platform layer may continue to own:

- iOS Keychain / Android Keystore integration;
- biometrics and system authentication;
- Realm/database encryption and persistence;
- app lock/session behavior;
- React Native UI and platform lifecycle.

Rust Core is authoritative for protected wallet-secret formats, key derivation, software/external signer semantics, identity verification, and signing.

Mobile stores Core-generated encrypted wallet envelopes as opaque data. System authentication authorizes signer use; it does not define a second wallet encryption format.

See [Mobile / Rust Core Vault Contract](mobile-core-contract.md) and [Wallet Protection Model](protection.md).

## Core Concepts

### Wallet

Responsible for:

- mnemonic handling;
- account derivation;
- wallet state semantics;
- key-management semantics.

Persistence of encrypted wallet state belongs to the client/platform storage layer.

### Transaction Intent

Transaction flow:

```text
Intent
  |
Build Transaction
  |
Review
  |
Sign
  |
Submit
```

### Signer

Signing is isolated behind an interface so implementations can support:

- software signer;
- hardware wallet;
- mobile/platform-backed external signer;
- future contract/passkey authorization.

System authentication is conceptually signer authorization and should remain independent from account identity and wallet cipher format.

## Rust Core clients

- Mobile
- Desktop
- CLI
- TUI
- SDK

Clients must consume the same Core wallet/signing semantics rather than creating parallel cryptographic implementations.

## History Cache

History is a network-and-account-scoped cache of raw Horizon operations, not a full-chain indexer. The default cache retains the newest 2,000 operations; users may opt into keeping all history that their Horizon source still exposes. Empty caches are built from the current head backwards, while later refreshes always move from the newest local cursor forward to the current head.

See [History Cache Model](history-cache.md) and the [Architecture Decision Log](decision-log.md).

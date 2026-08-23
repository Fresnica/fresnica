# Fresnica Architecture

## Current Phase

Python reference implementation.

The Python implementation defines wallet behavior and creates test vectors for future Rust implementation.

## Architecture

```
CLI / TUI
    |
Wallet API
    |
Stellar Adapter
    |
Stellar Network
```

## Core Concepts

### Wallet

Responsible for:

- mnemonic handling
- account derivation
- wallet state
- key management

### Transaction Intent

Transaction flow:

```
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

Signing is isolated behind an interface so future implementations can support:

- software signer
- hardware wallet
- mobile secure storage

## Future Rust Core

The Rust implementation will replace the Python engine after behavior is verified.

Clients:

- Mobile
- Desktop
- CLI
- TUI
- SDK


## History Cache

History is a network-and-account-scoped cache of raw Horizon operations, not a full-chain indexer. The default cache retains the newest 2,000 operations; users may opt into keeping all history that their Horizon source still exposes. Empty caches are built from the current head backwards, while later refreshes always move from the newest local cursor forward to the current head.

See [History Cache Model](history-cache.md) and the [Architecture Decision Log](decision-log.md).

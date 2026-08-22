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

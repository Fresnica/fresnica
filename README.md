# Fresnica

Fresnica is a self-custody Stellar wallet project.

## Vision

Build a wallet engine that can power multiple interfaces:

- Mobile
- Desktop
- CLI
- TUI
- SDK
- Agent interfaces

## Development Strategy

Phase 1: Python reference implementation.

The Python version is used to validate wallet logic, create test vectors, and define behavior.

Phase 2: Rust Fresnica Core.

The Rust implementation will become the production wallet engine used by different clients.

## Current First Goal

Implement:

```
Mnemonic
  -> Stellar HD derivation
  -> Address generation
  -> Wallet model
```

Then extend to:

- Balance
- Asset management
- Transaction building
- Signing
- Sending
- TUI wallet interface


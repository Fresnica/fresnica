# Fresnica Roadmap

## Phase 0 - Architecture Foundation

Completed:

- Wallet / Account / Signer separation
- Watch-only wallet concept
- Stellar SDK as protocol layer
- WalletManager concept
- DataStore abstraction
- Service layer architecture
- CLI and TUI separation

## Phase 1 - Python Wallet Runtime

Completed in the Python reference:

- Running CLI application
- Running TUI application
- Wallet lifecycle management
- Balance display
- Transfer flow
- Runtime dependency wiring
- Wallet storage
- SQLite datastore
- CLI commands
- Rich rendering
- Textual dashboard
- Transaction submit flow

## Phase 2 - Stellar Features

Implemented in the Python reference and increasingly ported to the native Rust CLI:

- Assets and portfolio view
- Standalone trustline add / limit / remove lifecycle
- Offers and pair-scoped SDEX terminal
- Fex-compatible trade aggregation
- History/cache model
- Contacts/address resolution
- Friendbot testnet funding

## Phase 3 - Production Core and Native Clients

### Completed

- Production Rust Core primitives
- Stable library-level `CoreClientApi` v2
- Account identity / signer capability separation
- Watch-only signer attachment identity checks
- Protected software signer import/generation/sign/reveal lifecycle
- Passcode re-protection without client-side secret disclosure
- `WalletUnlockKey` system-auth boundary
- External Ed25519 prepare/apply signing boundary
- Thin process adapter for Python/reference verification
- Native Rust CLI direct-link client

### Current

- FFI-neutral mobile binding foundation in `bindings/mobile`
- Fixed-width/string/byte mobile DTOs over `CoreClientApi`
- Mobile binding conformance tests and dedicated CI

### Next

1. Choose the concrete Swift/Kotlin generation/interop layer after the FFI-neutral DTO surface is green.
2. Build the native iOS/Android module around `fresnica-mobile-core`.
3. Integrate Xaman-derived account/signer persistence without reusing Xaman secret cryptography.
4. Implement Keychain/Keystore `WalletUnlockKey` enrollment and biometric release.
5. Add watch-only upgrade/downgrade and passcode-rotation mobile flows.
6. Add a first real hardware signer transport using the existing external Ed25519 prepare/apply API.
7. Continue desktop client work against the same Core/mobile-neutral facade where applicable.

## Phase 4 - Mobile Product Integration

Target product work after native binding generation is selected:

- Fresnica account/signer Realm schema
- Xaman-derived navigation and account management adaptation
- system-auth enrollment and recovery fallback
- software signing with no private key crossing React Native for routine use
- explicit Reveal / Export flow
- passcode rotation with staged/atomic re-protection
- hardware/external signer UX
- migration policy before first public wallet release

The mobile application must preserve the boundary in `mobile-core-contract.md`: platform code owns persistence and authorization policy; Rust Core owns cryptographic and signer semantics.

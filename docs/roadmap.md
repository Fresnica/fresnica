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
- FFI-neutral `fresnica-mobile-core` facade
- Fixed-width/string/byte mobile DTO and error boundary
- Mobile binding conformance tests
- Stable UniFFI 0.32.x selected for Swift/Kotlin generation
- Proc-macro UniFFI export of the same mobile facade
- Host Swift/Kotlin binding generation gate in CI

### Current

- Android Rust ABI packaging + generated Kotlin integration
- Apple Rust static libraries / XCFramework + generated Swift integration
- Thin React Native native modules over generated Swift/Kotlin APIs

### Next

1. Cross-compile `fresnica-mobile-core` for the supported Android ABIs and package generated Kotlin + native libraries.
2. Build Apple static libraries for simulator/device architectures and package the generated Swift FFI layer as an XCFramework-compatible dependency.
3. Add thin `FresnicaCoreModule` React Native adapters to the Xaman-derived application.
4. Integrate Xaman-derived account/signer persistence without reusing Xaman secret cryptography.
5. Implement Keychain/Keystore `WalletUnlockKey` enrollment and biometric release.
6. Add watch-only upgrade/downgrade and passcode-rotation mobile flows.
7. Add a first real hardware signer transport using the existing external Ed25519 prepare/apply API.
8. Continue desktop client work against the same Core/mobile-neutral facade where applicable.

## Phase 4 - Mobile Product Integration

Target product work after native binding packaging:

- Fresnica account/signer Realm schema
- Xaman-derived navigation and account management adaptation
- system-auth enrollment and recovery fallback
- software signing with no private key crossing React Native for routine use
- explicit Reveal / Export flow
- passcode rotation with staged/atomic re-protection
- hardware/external signer UX
- migration policy before first public wallet release

The mobile application must preserve the boundary in `mobile-core-contract.md`: platform code owns persistence and authorization policy; Rust Core owns cryptographic and signer semantics.

See `mobile-bindings.md` for the accepted UniFFI and React Native integration direction.

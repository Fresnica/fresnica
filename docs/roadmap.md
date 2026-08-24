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
- Stable UniFFI 0.32.x Swift/Kotlin generation
- Android four-ABI Rust package + generated Kotlin package
- Apple device/simulator Rust package + generated Swift/FFI XCFramework
- Native platform packaging CI/artifacts
- Per-signer user-auth-bound `WalletUnlockKey` storage on Android and Apple
- Core-backed native software-signer enrollment and biometric signing coordinators
- Thin React Native protected-signing bridge with no unlock-key exposure to JavaScript

### Current

- Xaman-derived mobile host integration for the `FresnicaCore` React Native module
- Fresnica AccountRecord / SignerRecord persistence in the mobile Realm/client layer
- Watch-only account lifecycle using signer attachment rather than account/private-key coupling

### Next

1. Integrate Fresnica AccountRecord / SignerRecord persistence without reusing Xaman secret cryptography.
2. Add mobile watch-only upgrade/downgrade while preserving stable account identity and metadata.
3. Add staged/atomic app-passcode rotation across all protected software signers.
4. Add explicit mobile Reveal / Export handling with a fresh app passcode.
5. Add a first real hardware signer transport using the existing external Ed25519 prepare/apply API.
6. Continue desktop client work against the same Core/mobile-neutral facade where applicable.

## Phase 4 - Mobile Product Integration

Target product work after native security/module integration:

- Fresnica account/signer Realm schema
- Xaman-derived navigation and account management adaptation
- system-auth enrollment and recovery fallback
- software signing with no private key or unlock key crossing React Native for routine use
- explicit Reveal / Export flow
- passcode rotation with staged/atomic re-protection
- hardware/external signer UX
- migration policy before first public wallet release

The mobile application must preserve the boundary in `mobile-core-contract.md`: platform code owns persistence and authorization policy; Rust Core owns cryptographic and signer semantics.

See `mobile-bindings.md` for the UniFFI/React Native layering and `mobile-system-auth.md` for the native WalletUnlockKey policy.

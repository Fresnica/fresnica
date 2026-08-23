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

Goals:

- Running CLI application
- Running TUI application
- Wallet lifecycle management
- Balance display
- Transfer flow

Tasks:

- Runtime dependency wiring
- Wallet storage
- SQLite datastore
- CLI commands
- Rich rendering
- Textual dashboard
- Transaction submit flow

## Phase 2 - Stellar Features

Implemented in the Python reference:

- Assets and portfolio view
- Standalone trustline add / limit / remove lifecycle
- Offers and pair-scoped SDEX terminal
- Fex-compatible trade aggregation

## Phase 3 - Production Core

- Rust Fresnica Core
- Mobile bindings
- Desktop application
- SDK API
- Hardware signer support

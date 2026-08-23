# Fresnica Tasks

## Runtime

- [x] Compose Runtime dependencies
- [x] Per-network service composition
- [x] Application home/config root (`FRESNICA_HOME`)
- [x] User configuration file

## Wallet

- [x] Account model
- [x] Signer abstraction
- [x] Watch-only wallet
- [x] Encrypted mnemonic/secret storage
- [x] File wallet storage
- [x] Lock/unlock lifecycle
- [x] Wallet create/import/list/use/delete commands
- [x] Export/backup workflow
- [x] Verified external Ed25519 signer adapter
- [ ] Hardware transport adapters

## Data

- [x] DataStore abstraction
- [x] SQLite implementation
- [x] Network-isolated raw balance cache
- [x] Raw operation/history cache
- [x] Offers cache
- [x] Trades cache
- [x] Fex-style trade aggregation / market data

## CLI

- [x] `fresnica` -> TUI
- [x] Rich renderer
- [x] `balance`
- [x] `history`
- [x] `send AMOUNT ASSET to DESTINATION`
- [x] `info`
- [x] wallet lifecycle commands
- [x] Human transaction review + Y/n confirmation
- [x] `--json` balance output
- [x] Contact/address book integration

## TUI

- [x] Textual application entry
- [x] Wallet/balance dashboard
- [x] Background balance refresh
- [x] Wallet switcher
- [x] History screen
- [x] Send/unlock/review modal flow
- [x] Pair-scoped SDEX screen

## Transaction

- [x] Asset parsing
- [x] Reserve/liability-aware available amount
- [x] Transaction build via Stellar SDK
- [x] Sign flow
- [x] Submit flow
- [x] User-facing transaction result
- [x] Result/history refresh after submit
- [x] Memo-required account handling

## Quality

- [x] Unit tests for storage, encryption, cache, CLI parsing, availability, runtime, and transfer orchestration
- [x] GitHub Actions test workflow
- [x] Real Horizon integration tests on Testnet
- [x] Cross-language test vectors for future Rust Core

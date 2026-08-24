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
- [x] `trust add/limit/remove`
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
- [x] Trustline management screen

## Transaction

- [x] Asset parsing
- [x] Reserve/liability-aware available amount
- [x] Standalone trustline add/limit/remove workflow
- [x] Transaction build via Stellar SDK
- [x] Sign flow
- [x] Submit flow
- [x] User-facing transaction result
- [x] Result/history refresh after submit
- [x] Memo-required account handling

## Production Core

- [x] Rust wallet derivation and software signer primitives
- [x] Versioned protected signer envelope
- [x] Verified `WalletUnlockKey` derivation and signing
- [x] Explicit Reveal / Export boundary
- [x] Account `G...` / `C...` identity parsing
- [x] Account identity / signer capability separation
- [x] Library-level `CoreClientApi` v2
- [x] Thin `fresnica-core` process adapter
- [x] Watch-only signer attachment identity verification
- [x] Core-side passcode re-protection
- [x] External Ed25519 prepare/apply signing boundary
- [x] Native Rust CLI direct-link client

## Mobile Binding

- [x] FFI-neutral mobile facade crate (`bindings/mobile`)
- [x] Fixed-width / String / byte-array DTO boundary
- [x] Stable mobile error mapping from `ClientApiErrorCode`
- [x] 32-byte unlock-key boundary validation
- [x] Shared transaction-vector conformance tests
- [x] Dedicated mobile binding CI
- [x] Select stable UniFFI 0.32.x for Swift/Kotlin generation
- [x] Export mobile facade with UniFFI proc macros
- [x] Generate Swift and Kotlin bindings from compiled library metadata in CI
- [ ] Package Android Rust libraries for supported ABIs
- [ ] Compile/package generated Kotlin API in Android build
- [ ] Build Apple simulator/device static libraries
- [ ] Package generated Swift API + Rust static libraries for Xcode/XCFramework use
- [ ] Add React Native native-module adapter
- [ ] Keychain/Keystore unlock-key enrollment
- [ ] Biometric signer authorization flow
- [ ] Mobile watch-only upgrade/downgrade lifecycle
- [ ] Mobile staged/atomic app-passcode rotation
- [ ] Explicit mobile Reveal / Export flow

## Quality

- [x] Unit tests for storage, encryption, cache, CLI parsing, availability, runtime, and transfer orchestration
- [x] GitHub Actions test workflow
- [x] Real Horizon integration tests on Testnet
- [x] Cross-language test vectors for Rust Core
- [x] Rust Core / Python process conformance coverage
- [x] Mobile binding conformance workflow
- [x] Host Swift/Kotlin UniFFI generation gate

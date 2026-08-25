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
- [x] Ledger hardware-provider boundary/dependency review (`docs/hardware-signer.md`)

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
- [x] Native Rust CLI direct SDK/Core reference client
- [x] Native Rust CLI watch-only signer attach/detach lifecycle with expected-signer verification
- [x] Native Rust CLI SEP-1 + SEP-6/SEP-24 anchor capability discovery (`anchor discover CODE:GISSUER`)
- [x] Native Rust CLI SEP-10/SEP-45 authentication metadata discovery and G/C capability classification
- [ ] Native Rust CLI verified anchor authentication/session boundary before SEP-6/SEP-24 authenticated execution

## Universal / Native SDK

- [x] Platform-neutral `fresnica-sdk` semantic contract
- [x] Mobile v0.1.0 compatibility facade delegates to `fresnica-sdk`
- [x] Generalized `fresnica-native-sdk` UniFFI binding
- [x] Android Native SDK AAR without framework adapter code
- [x] Standalone Android raw-AAR consumer compile gate + explicit host dependency contract
- [x] Apple Native SDK Rust FFI XCFramework/package without framework adapter code
- [x] Validate compiled importable `FresnicaSDK.xcframework` packaging on real macOS/Xcode (`validate-apple-local.sh`, 2026-08-25)
- [x] Android Keystore/native signer-authorization helpers in Native SDK package
- [x] Apple Keychain/LocalAuthentication signer-authorization helpers in Native SDK package
- [x] Desktop direct-consumer contract and supported-surface rules
- [x] Validate implemented Apple Swift Native SDK macOS slices on real Xcode (`validate-apple-local.sh`, 2026-08-25)
- [ ] Windows/Linux non-Rust package only after a concrete consumer language/framework is selected
- [x] Canonical React Native adapter source targets `fresnica-native-sdk` instead of the transitional Mobile facade
- [x] React Native Android one-time adapter build entry point + compatibility manifest/rebuild diagnostics
- [x] Apple one-time React Native adapter binary build path targets compiled `FresnicaSDK`
- [x] One-command real-consumer Apple RN adapter validation wrapper (`adapters/react-native/apple/validate-consumer.sh`)
- [x] Validate static `FresnicaRNAdapter.xcframework` against a real RN 0.87 CocoaPods consumer on macOS (`validate-consumer.sh`, 2026-08-25)
- [x] WASM/Web API filtering and fresh-passcode routine-signing security boundary
- [x] WASM source-boundary and generated-package surface tests
- [x] WASM target/package compile validation
- [x] Browser-package runtime/shared-vector conformance harness
- [x] Machine-readable SDK compatibility manifest + source drift validator
- [x] Native SDK release contract + marker-gated release automation (`native-sdk-v*`; no marker/release published yet)
- [x] Execute WASM package/runtime conformance with a Rust + wasm-bindgen toolchain
- [x] Passkey/smart-account architecture decision: contract-account signer, not WalletUnlockKey wrapper
- [x] Pinned `smart-account-kit` provider boundary + Testnet deployment fixture + mock conformance tests
- [x] Localhost WebAuthn/Testnet smoke harness for create/fund/discover/transfer
- [x] Real-Testnet auth-XDR capture + Protocol-27/context-rule/WebAuthn fixture verifier harness
- [x] Real browser/Testnet passkey create/connect/sign-and-submit smoke validation
- [x] Smart-account auth-XDR/context-rule conformance fixture from real Testnet transaction (`spec/test-vectors/smart-account-auth-v1.json`)
- [ ] Platform-native Mobile passkey provider after the Testnet provider boundary is proven

## Mobile Binding

- [x] FFI-neutral mobile facade crate (`bindings/mobile`)
- [x] Fixed-width / String / byte-array DTO boundary
- [x] Stable mobile error mapping from `ClientApiErrorCode`
- [x] 32-byte unlock-key boundary validation
- [x] Shared transaction-vector conformance tests
- [x] Dedicated mobile binding CI
- [x] Stable UniFFI 0.32.x Swift/Kotlin generation
- [x] Export mobile facade with UniFFI proc macros
- [x] Generate Swift and Kotlin bindings from compiled library metadata in CI
- [x] Package Android Rust libraries for `armeabi-v7a`, `x86`, `x86_64`, `arm64-v8a`
- [x] Package generated Kotlin source with Android native libraries
- [x] Build Apple device and arm64/x86_64 simulator static libraries
- [x] Package generated Swift API + Rust FFI static libraries for XCFramework/Xcode use
- [x] Native platform packaging CI/artifacts
- [x] Android auth-per-use AES-GCM WalletUnlockKey storage primitive
- [x] Apple `ThisDeviceOnly + biometryCurrentSet` WalletUnlockKey storage primitive
- [x] Wire Keychain/Keystore enrollment to Core `derive_unlock_key`
- [x] Add React Native native-module adapter
- [x] Native-only biometric `sign_transaction_xdr` orchestration
- [x] Mobile AccountRecord / SignerRecord lifecycle coordinator
- [x] Mobile watch-only upgrade/downgrade lifecycle
- [x] Realm-ready Account / Signer / reference schemas and WalletStore adapter
- [x] Mobile staged/atomic app-passcode rotation
- [x] Protected account secret/mnemonic import and mnemonic generation provisioning
- [x] Explicit mobile signer Reveal / Export product flow
- [ ] Wire the host application's Realm instance and migrations to `RealmWalletStore`

## Quality

- [x] Unit tests for storage, encryption, cache, CLI parsing, availability, runtime, and transfer orchestration
- [x] GitHub Actions test workflow
- [x] Rust CLI SDK-boundary guard + unit/release/Python-compat CI validation
- [x] Real Horizon integration tests on Testnet
- [x] Cross-language test vectors for Rust Core
- [x] Rust Core / Python process conformance coverage
- [x] Mobile binding conformance workflow
- [x] Host Swift/Kotlin UniFFI generation gate
- [x] Android/Apple native package build gate
- [x] Android/Apple system-auth support compile gate
- [x] Android/Apple React Native protected-signing bridge build gate
- [x] React Native account/signer lifecycle TypeScript gate
- [x] CI path isolation between JS lifecycle, native platform, and Rust/UniFFI packaging work

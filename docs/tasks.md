# Fresnica Tasks


## Documentation / Cross-Platform Contracts

- [x] Canonical cross-project vocabulary: Application Flows / Application Capabilities / Fresnica Core / Infrastructure Ports
- [x] Application Flow contract (`docs/application-flows.md`)
- [x] Application Capability contract and maturity model (`docs/application-capabilities.md`)
- [x] Core security boundary (`docs/core-security-boundary.md`)
- [x] Platform implementation contract (`docs/platform-implementation.md`)
- [x] Documentation index and directory reorganization (`docs/README.md`)
- [x] Per-directory documentation entry points for Capabilities/Core/Platforms/SDK/Development
- [x] Detailed Capability contracts/reference pages under `docs/capabilities/`
- [x] Shared cross-capability domain primitives for network/account/signer/asset/amount/price semantics
- [x] Capability maturity audit: keep premature Wallet/History/Contacts semantics Defined rather than freezing terminal implementation shapes
- [x] Reference Semantics model for Defined capabilities, including implementation/test evidence and explicit implementation-specific exclusions
- [x] Extract RefPython History/Wallet/Contacts behavior into Capability references instead of leaving proven semantics hidden in reference code
- [x] Record Native SDK Apple/Android system-auth behavior as Application Security Reference Semantics without prematurely freezing product APIs
- [x] Allow independent product repositories such as `fresnica-mobile` to mature shared contracts through evidence-backed documentation PRs
- [x] Compact `docs/handoff.md` so stable architecture rules live only in the common contracts
- [x] Move Python reference product documentation out of the root project README
- [x] Preserve `fresnica-client` as Rust reference Capability implementation without requiring other platforms to link it
- [x] Shared application error-semantics vocabulary across Core/Capability/Flow boundaries
- [x] Defined Asset Discovery / Catalog Capability from RefPython cache/discovery evidence
- [x] Allow non-normative Reference extensions to mature already-Normative Capabilities
- [x] Define evidence-backed cross-repository contract PR expectations for Mobile/Web/Desktop implementations
- [x] Promote confirmed-transaction-success vs post-submit-refresh failure isolation into the Flow contract
- [x] Defined Backup / Restore Capability from shared Rust/RefPython encrypted v1 evidence without freezing the terminal record schema
- [x] Defined Ledger Authorization Capability separating local signer availability from actual prepared-transaction ledger authorization
- [x] Shared Recovery Source, memo, Muxed-address and exact-case asset identity vocabulary
- [x] Record current Transaction TxV1 scope and fail-closed Fee Bump / unknown-Dapp review boundary
- [x] Record CAP-18 trustline authorization differences and current-protocol orphaned-issuer semantics
- [x] Record new-offer reserve capacity as a pre-crossing requirement even when an offer fully matches and leaves no residual OfferEntry

### Implementation follow-ups discovered by contract audit

- [x] Align Rust SDEX decimal-price rationalization with the canonical bounded best-rational behavior, including semi-convergent recovery at signed-int32 boundaries (for example `2147483648` -> `2147483647/1`)
- [x] Make Rust/RefPython SDEX BUY preflight derive price-dependent selling capacity from the same effective `Price { n, d }` that will be encoded, not independently from the requested decimal
- [x] Align Rust/RefPython SDEX create/update review with the exact encoded Stellar `Price { n, d }` when decimal input requires rational approximation; requested price alone is not sufficient review truth
- [x] Extend SDEX conformance vectors with exact, approximated and signed-int32-boundary decimal-price rationalization cases
- [x] Expose the effective trustline limit in Rust/RefPython SDEX review whenever offer preparation adds a receiving trustline; the operation already uses the canonical Fresnica marker but current review only carries the asset
- [x] Clean legacy RefPython lower-level trustline builder/help/test wording that still says `Stellar maximum`; current `TrustlineService` already supplies the canonical Fresnica `708269837873.6765` marker
- [x] Rework Rust/RefPython SDEX preflight around final ledger effect: exact integer-stroop liabilities/rounding, receiving capacity, issuer special cases, replacement liabilities on update, and fee/authorization handling on cancel
- [x] Add Balance/Payment receiving-capacity semantics in Rust/RefPython, including issued trustline limit/buying-liability headroom, native `INT64_MAX` headroom and issuer-own-asset special handling
- [x] Add Payment source/destination trustline full-authorization and destination-capacity preflight plus explicit SEP-29 `memo_required` protection to the Rust shared Payment path; keep issuer-special/orphaned-issuer behavior aligned with current protocol
- [x] Add CAP-18 authorization-aware SDEX preflight: create/update require full authorization while cancel remains valid with maintain-liabilities authorization
- [x] Add Trustline remove preflight for `liquidityPoolUseCount`, issuer-existence add/nonzero-limit rules, and resulting authorization/clawback state; keep pool-share ChangeTrustAsset outside current product scope
- [x] Reject signing of already-expired prepared transactions and require re-prepare + re-review; add regression coverage for timebound expiry
- [x] Make Contacts resolution prefer a valid direct chain identity over alias lookup so an address-like contact cannot shadow a pasted destination
- [x] Harden Rust/RefPython Anchor asset matching to exact-case full identity and reject automatic redirects so an initially validated HTTPS endpoint cannot silently change transport/origin
- [x] Make direct-Classic SEP-10 fail explicitly before signing when the available local signer cannot satisfy the supported direct path: Rust checks current master weight against Horizon medium threshold and RefPython rejects an attached delegated signer
- [x] Route Rust Classic SEP-10 through reusable Ledger Authorization + Signing Coordination for local software Ed25519 multisig, including medium-threshold evaluation and server-key exclusion
- [x] Add cross-language asset-identity vectors for protocol-valid case-sensitive issued codes, including values that high-level SDK convenience constructors may normalize
- [x] Add the first reusable Rust Ledger Authorization planning slice for normalized typed Horizon signers/thresholds, transaction + operation sources, weighted availability and fail-closed unsupported semantics
- [x] Route the Rust reference shared submit path through fresh Horizon Ledger Authorization and coordinate only the local software Ed25519 signatures still required after existing signatures/preauth conditions are evaluated
- [ ] Extend the proven Rust local-Ed25519 Ledger Authorization path to provider-backed Hash-X/signed-payload/external signer conditions before claiming general Classic multisig/delegated signing support
- [ ] Define a next-generation portable Backup/Restore format/activation path before Mobile adoption; authenticate or independently revalidate security-significant account/signer/network relationship metadata rather than copying terminal v1 wholesale

## Runtime

- [x] Compose Runtime dependencies
- [x] Per-network Capability composition
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
- [x] Ledger hardware-provider boundary/dependency review (`docs/capabilities/external-signer.md`)

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
- [x] RefPython / Stellar-SDK memo-required account handling

## Production Core

- [x] Rust wallet derivation and software signer primitives
- [x] Versioned protected signer envelope
- [x] Verified `WalletUnlockKey` derivation and signing
- [x] Explicit Reveal / Export boundary
- [x] Account `G...` / `C...` identity parsing
- [x] Account identity / signer capability separation
- [x] Library-level `CoreClientApi` v3
- [x] Thin `fresnica-core` process adapter
- [x] Watch-only signer attachment identity verification
- [x] Core-side passcode re-protection
- [x] External Ed25519 prepare/apply signing boundary
- [x] Native Rust CLI direct SDK/Core reference client
- [x] Reusable Rust Application Capability/reference client crate shared by CLI/TUI (`clients/rust-client`)
- [x] First native Rust TUI dashboard over shared Rust Capability implementations (`clients/rust-tui`)
- [x] Shared UI-free Rust transaction/payment prepare-review-submit Capability implementations with pending retry protection
- [x] First native Rust TUI reviewed payment write flow over shared Rust Capability implementations
- [x] Shared UI-free Rust trustline prepare-review-submit Capability implementations
- [x] Native Rust TUI trustline add/limit/remove flow over shared Rust Capability implementations
- [x] Shared UI-free Rust SDEX offer create/update/cancel Capability implementations
- [x] Native Rust TUI SDEX offer create/update/cancel flow over shared Rust Capability implementations
- [x] Shared typed SDEX open-offer read Capability implementation with CLI/TUI consumers
- [x] Shared typed SDEX order-book Capability implementation with exact `price_r` semantics
- [x] Shared typed pair-trade/account-fill/candle Capability implementations
- [x] Native Rust TUI pair market view with order book, recent trades and candles
- [x] Native Rust CLI watch-only signer attach/detach lifecycle with expected-signer verification
- [x] Native Rust CLI SEP-1 + SEP-6/SEP-24 anchor capability discovery (`anchor discover CODE:GISSUER`)
- [x] Native Rust CLI SEP-10/SEP-45 authentication metadata discovery and G/C capability classification
- [x] Native Rust CLI verified SEP-10 challenge/session boundary before authenticated anchor execution
- [x] Native Rust CLI SEP-24-preferred / SEP-6-fallback deposit and withdraw initiation
- [x] Native Rust CLI reviewed withdrawal payment handoff and anchor transaction-status tracking
- [x] Native Rust CLI SEP-12 customer status/common field update handoff for anchor KYC/update states
- [x] Shared Rust Anchor Capability implementation for SEP-1 discovery, Classic SEP-10 challenge/session, SEP-6/SEP-24 transfer transport and transaction status
- [ ] SEP-12 nested structured values and optional `/customer/files` file-id workflow when a concrete anchor requires them

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
- [x] Native SDK release contract + marker-gated release automation (`native-sdk-v*`)
- [x] First generalized Native SDK integration release (`native-sdk-v0.1.0`)
- [x] Mobile security/HD Native SDK v0.2 contract: Core/SDK `derive_mnemonic_signer`, API 3/3/2, RN adapter 0.2.0
- [x] Publish `native-sdk-v0.2.0` from PR #109 / `0de8be4` after Android raw-AAR consumer + Apple direct-consumer + native-signing gates
- [x] Native SDK v0.2.1 corrective handoff: make Android/Apple System Auth Domain commit/cleanup failure semantics recoverable and synchronize the Mobile v0.2 documentation/release pin
- [x] Device-level System Auth Protection Domain: one authenticated domain initialization, later passcode-verified signer registration without repeat biometric prompts
- [x] Android RSA-OAEP auth-bound domain + public-key per-signer WalletUnlockKey wrapping
- [x] Apple P-256/ECIES auth-bound domain + public-key per-signer WalletUnlockKey wrapping
- [x] Preserve `Passcode > System Auth`; Reveal/Export and passcode rotation remain passcode-authorized
- [x] Atomic all-signer passcode rotation contract followed by no-biometric wrapped-key replacement in the existing domain
- [x] Retire legacy `mobile-sdk-v*` publisher from active `main` workflows
- [x] Mobile Native SDK / React Native adapter onboarding guide (`docs/platforms/mobile/sdk-usage.md`)
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
- [x] Legacy Android per-signer auth-per-use AES-GCM WalletUnlockKey primitive (superseded by Native SDK v0.2 device System Auth Domain)
- [x] Legacy Apple per-signer `ThisDeviceOnly + biometryCurrentSet` WalletUnlockKey primitive (superseded by Native SDK v0.2 device System Auth Domain)
- [x] Wire Native SDK v0.2 domain signer registration to Core verified unlock-key derivation; public-key wrapping requires no repeat biometric prompt
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

# Fresnica Tasks

Updated: 2026-09-03

This list tracks shared-repository work only. Independent product implementation belongs in the corresponding product repository.

## Current shared-repository priorities

### Repository boundary cleanup

- [x] Reclassify `Fresnica/fresnica` as Core / SDK / Specification / Reference rather than a wallet product monorepo
- [x] Move the Rust Capability implementation from `clients/rust-client` to `reference/rust-client`
- [x] Move the Rust SDK-boundary guard to repository-level `scripts/validate-rust-sdk-boundary.sh`
- [x] Remove the stale direct `fresnica-core` dependency from the Rust CLI and keep product layers behind SDK/reference semantics
- [x] Stop `Required CI` from compiling CLI/TUI as immediate shared-contract dependencies; keep their dedicated workflows during migration
- [x] Create `Fresnica/fresnica-terminal` and migrate Rust CLI + TUI together
- [x] After independent terminal CI is green, remove terminal product source and product workflows from this shared repository
- [x] Enable repository ruleset/branch protection requiring PRs and the stable required CI; prohibit force-push/deletion
- [ ] Retire historical probe/relay/validation branches through an admin-capable path

### Modern Stellar Core Capability Baseline

- [x] Adopt the Modern Stellar Core Capability Baseline as the staged evolution target
- [x] Keep G-account identity, C-account identity and signer capability separate
- [x] Compile against maintained Protocol-28-ready Stellar XDR ahead of Mainnet activation
- [x] Add Soroban authorization XDR parsing/encoding with bounded decode behavior
- [x] Add legacy Address and CAP-71 AddressV2 authorization preimage/hash semantics
- [x] Add direct G-account Ed25519 Soroban authorization signing and verification
- [x] Fail closed for C-account/custom/delegated authorization until a concrete provider exists
- [x] Add a language-neutral Soroban authorization signing conformance vector
- [x] Expose protected/external Soroban authorization through `CoreClientApi` with stable `invalid-authorization` errors
- [x] Expose the proven Soroban authorization contract through the platform-neutral SDK
- [x] Extend Process Binding to SDK v4 Soroban authorization only for the concrete RefPython consumer; keep Native/WASM demand-driven
- [x] Add SEP-53 v1.0.0 message signing/verification as a separate Core/SDK domain with language-neutral vectors
- [x] Expose SEP-53 through Native Binding API 3 and the React Native system-auth/passcode bridge for the concrete Mobile dapp challenge consumer
- [x] Use RefPython to prove Soroban simulation/assembly/review semantics
- [x] Prove source-account and detached Classic G-account Soroban authorization/signing plus Testnet submit/status reconciliation in RefPython
- [x] Add RustClient RPC/gateway + Soroban transaction capability after RefPython evidence
- [ ] Add a concrete smart-account/C-account provider before extracting a generic provider interface

### Security

- [x] Classify Process Binding as a privileged owner/host surface rather than an Agent, remote, renderer or plugin API
- [ ] Review whether Process Binding owner-only `reveal` and `derive-unlock-key` operations need a narrower profile before any non-RefPython Desktop consumer ships
- [ ] Add executable Backup v1 metadata-mutation regressions and keep v1 terminal/legacy-only
- [ ] Pin Fresnica-owned release Actions/toolchains/dependencies where appropriate
- [ ] Add dependency audit, SBOM and provenance/attestation to the Native SDK release path

### Agent integration — reuse, do not duplicate

- [x] Evaluate Soneso Stellar Agent Wallet as the preferred Agent execution/policy/MCP layer
- [x] Keep Fresnica as protected signer authority rather than duplicating Agent policy/approval/audit/network logic
- [x] Reject a raw-hash signer adapter and decrypting Fresnica material into the upstream keyring
- [ ] Propose/land an upstream application-level exact-envelope signer backend while preserving default keyring behavior
- [ ] Validate returned signed envelopes cannot mutate the approved transaction
- [ ] Implement the narrow Fresnica protected-signer backend after the upstream seam exists
- [ ] Prove one bounded Testnet Payment through upstream MCP/policy/approval/audit and Fresnica exact-XDR signing
- [ ] Keep the current operation-type `AgentCapability` dormant; replace/remove it rather than promoting a parallel policy engine

### Protocol/provider work — demand-driven

- [x] Add an opt-in RefPython Ledger Stellar HID provider over the existing Core external Ed25519 prepare/apply boundary, with deterministic APDU/path/chunking tests
- [x] Run the Ledger provider on a physical macOS-connected device against Testnet with Ledger Stellar app 6.0.3, path `m/44'/148'/0'`, Blind Signing disabled, and record transaction `f91abc8bd8af37484bbb0c3c0e933e454df3131bb6b21598715e3af8f2beb4b0`
- [ ] Add provider-backed collection for Hash-X, signed-payload and external signer conditions before claiming general Classic multisig/delegated signing support
- [ ] Add SEP-12 nested structured values and optional `/customer/files` file-ID workflow only when a concrete anchor requires them
- [ ] Add product hardware transport adapters only after a concrete product needs them; reuse the proven RefPython/Core boundary rather than moving HID into Core
- [ ] Add Windows/Linux non-Rust packaging only after a concrete consumer language/framework is selected

## Established architecture and contracts

- [x] Canonical vocabulary: Application Flows, Application Capabilities, Fresnica Core and Infrastructure Ports
- [x] Five common contracts under `docs/`: architecture, flows, capabilities, Core security and platform implementation
- [x] Nineteen-capability catalog with maturity and Reference Semantics rules
- [x] Account identity, Signer capability and Recovery Source modeled as separate concepts
- [x] Network-scoped identity/state, exact asset identity, amount/price, memo and shared error semantics
- [x] RefPython governed as an executable product-semantics laboratory with explicit security boundaries
- [x] Evidence-backed cross-repository contract contribution process

## Core / SDK baseline

- [x] Rust software-signer derivation, SEP-0005 mnemonic handling and account identity parsing
- [x] Versioned Scrypt + AES-256-GCM protected signer envelope
- [x] Verified `WalletUnlockKey` derivation and protected transaction signing
- [x] Explicit passphrase-only Reveal / Export boundary
- [x] External Ed25519 transaction prepare/apply boundary with signature verification
- [x] Finite XDR decoding depth/input bounds and fail-closed unsupported envelope handling
- [x] Formal SDK Process Binding API v2; v1 owner operations retained and SDK v4 Soroban authorization transport added for RefPython
- [x] Machine-readable SDK compatibility manifest and source-drift validation
- [x] Native SDK release contract and published `native-sdk-v0.3.0` baseline (Native Binding API 3 / SDK API 5 / Core Client API 5)

## Rust capability reference implementation

- [x] Reusable `reference/rust-client` implementation shared by CLI/TUI
- [x] Wallet lifecycle and watch-only/local-signer enforcement
- [x] Balance/availability, Payment, Trustline and transaction prepare-review-submit
- [x] SDEX BUY/SELL intent preservation, exact `price_r`, offers, order book, trades, fills and candles
- [x] Ledger Authorization and Signing Coordination for local Ed25519 multisig paths
- [x] `PreconditionsV2.extraSigners` and recognition of existing Ed25519, preauth, Hash-X and signed-payload material
- [x] Pending/uncertain-submission guard and confirmed-success/post-refresh isolation
- [x] Native Rust CLI and TUI live in `Fresnica/fresnica-terminal` and consume the same capability layer through a pinned shared-repository revision

## Network / Anchor baseline

- [x] `HorizonGateway` identified as the current provider adapter, not a permanent shared contract
- [x] Shared Classic Asset identity based on a thin wrapper over official `stellar_xdr::Asset`
- [x] Central Anchor HTTPS/no-redirect/DNS/timeout/body-limit transport policy
- [x] SEP-1 discovery, Classic SEP-10, SEP-24-preferred / SEP-6-fallback initiation and status
- [x] Reviewed withdrawal handoff and common scalar/binary SEP-12 customer updates
- [x] Exact-case full asset identity and safe legacy SEP-6 metadata compatibility
- [x] Staged Horizon-to-RPC/Portfolio migration rationale and gateway normalization boundary

## Backup / recovery baseline

- [x] Backup/Restore Capability and terminal Backup v1 evidence documented
- [x] Backup/Restore v2 relationship model with backup-local Account/Signer references
- [x] Callback-driven revalidation before activation; unresolved relationships remain inactive
- [x] Target-network confirmation and signer identity/re-protection requirements
- [ ] Do not promote Backup v1 as a portable product format because outer metadata is unauthenticated

## Historical Mobile binding

- [x] `bindings/mobile` served as the v0.1 compatibility facade and validation donor
- [x] Native SDK / UniFFI plus framework adapters became the authoritative native integration path
- [x] Legacy Mobile binding source, publisher and active compatibility CI were retired from `main`
- [x] Historical tags, release artifacts and archived migration documentation preserve the old contract

## Quality and governance

- [x] Main bundle artifact workflow for isolated development recovery
- [x] Use a portable pinned rustfmt tool locally before pushing Rust changes
- [x] Keep stable `Required CI / validate` while formatting changed Rust files and testing only affected immediate Rust contracts
- [x] Defer expensive Native/Apple/Android/WASM/RefPython/CLI/TUI integration to final non-draft PR validation
- [x] Enable repository ruleset/branch protection requiring PRs and `Required CI / validate`; prohibit force-push and branch deletion

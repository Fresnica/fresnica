# Fresnica Tasks

Updated: 2026-09-01

This list tracks shared-repository work only. Independent product implementation belongs in the corresponding product repository.

## Current shared-repository priorities

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
- [ ] Expose the proven Soroban authorization contract through the platform-neutral SDK
- [ ] Extend Native/Process/WASM bindings only after the SDK contract is stable and required by consumers
- [ ] Add standard message signing/verification as a separate domain with SEP-53 alignment
- [ ] Use RefPython to prove Soroban simulation/assembly/review semantics
- [ ] Add RustClient RPC/gateway + Soroban transaction capability after RefPython evidence
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

- [ ] Add provider-backed collection for Hash-X, signed-payload and external signer conditions before claiming general Classic multisig/delegated signing support
- [ ] Add SEP-12 nested structured values and optional `/customer/files` file-ID workflow only when a concrete anchor requires them
- [ ] Add hardware transport adapters only after a concrete provider and exact-XDR compatibility gate exist
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
- [x] Formal SDK Process Binding API v1; duplicate Core/SDK bridge binaries retired
- [x] Machine-readable SDK compatibility manifest and source-drift validation
- [x] Native SDK release contract and published `native-sdk-v0.2.1` baseline

## Shared Rust capability implementation

- [x] Reusable `clients/rust-client` implementation shared by CLI/TUI
- [x] Wallet lifecycle and watch-only/local-signer enforcement
- [x] Balance/availability, Payment, Trustline and transaction prepare-review-submit
- [x] SDEX BUY/SELL intent preservation, exact `price_r`, offers, order book, trades, fills and candles
- [x] Ledger Authorization and Signing Coordination for local Ed25519 multisig paths
- [x] `PreconditionsV2.extraSigners` and recognition of existing Ed25519, preauth, Hash-X and signed-payload material
- [x] Pending/uncertain-submission guard and confirmed-success/post-refresh isolation
- [x] Native Rust CLI and engineering/reference TUI over the same capability layer

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
- [ ] Enable repository ruleset/branch protection requiring PRs and `Required CI / validate`; prohibit force-push and branch deletion

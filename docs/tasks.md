# Fresnica Tasks

This file tracks current shared-repository work. Product-host implementation belongs to independent product repositories; completed detail remains available in Git history, capability contracts, release notes and archived handoffs.

## Current shared-repository priorities

### Security

- [x] Classify Process Binding as a privileged owner/host surface rather than an Agent, remote, renderer or plugin API
- [ ] Review whether Process Binding owner-only `reveal` and `derive-unlock-key` operations need a narrower profile before any non-RefPython Desktop consumer ships
- [ ] Add executable Backup v1 metadata-mutation regressions and keep v1 terminal/legacy-only
- [ ] Pin Fresnica-owned release Actions/toolchains/dependencies where appropriate
- [ ] Add dependency audit, SBOM and provenance/attestation to the Native SDK release path
- [x] Evaluate Soneso Stellar Agent Wallet as the preferred Agent execution/policy/MCP layer and document the no-duplication boundary
- [ ] Propose an exact-envelope Classic signing backend to the upstream Stellar Agent Wallet while preserving its default keyring behavior
- [ ] Implement a narrow Fresnica protected-signer backend only after that exact-envelope seam exists
- [ ] Prove one bounded Testnet Payment through upstream MCP/policy/approval/audit and Fresnica exact-XDR signing

### Protocol and provider work — demand-driven

- [ ] Add provider-backed collection for Hash-X, signed-payload and external signer conditions before claiming general Classic multisig/delegated signing support
- [ ] Add SEP-12 nested structured values and optional `/customer/files` file-ID workflow only when a concrete anchor requires them
- [ ] Add hardware transport adapters only after a concrete provider and exact-XDR compatibility gate exist
- [ ] Add Windows/Linux non-Rust packaging only after a concrete consumer language/framework is selected

### External product evidence requests

These are shared-contract inputs, not implementation tasks for this repository.

- [ ] Independent products apply the shared new-passphrase/re-protection policy in onboarding/settings; credential-strength policy remains in Wallet/Application rather than Core
- [ ] Independent iOS/Android products benchmark current v1 Scrypt cost and report device/OS, latency and peak-memory evidence; products must not change KDF parameters without a versioned migration
- [ ] A platform-native Mobile passkey provider remains product-owned after the Testnet provider boundary is proven

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
- [x] External Ed25519 prepare/apply boundary with signature verification
- [x] Finite XDR decoding depth/input bounds and fail-closed unsupported envelope handling
- [x] `CoreClientApi` v3 and platform-neutral `fresnica-sdk` API v3
- [x] Native SDK / UniFFI API v2 with Android AAR and Apple XCFramework packaging
- [x] Filtered WASM binding and browser/shared-vector conformance
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
- [x] Shared Asset identity based on a thin wrapper over official `stellar_xdr::Asset`
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

## Agent Access status

- [x] Dormant Core prototype proves exact-envelope authorization-then-sign and fail-closed source/network/fee/count/type checks
- [x] CodebaseMemory/source audit confirms no production SDK, binding or transport consumer
- [x] Reuse assessment confirms that Stellar Agent Wallet already owns MCP, policy, approval, nonce/replay, audit, RPC construction and submission
- [x] Decide that Fresnica remains the protected signer authority and must not duplicate the upstream Agent stack
- [x] Reject a raw-hash `Signer` adapter because it would weaken Fresnica exact-XDR/network signing semantics
- [x] Reject decrypting Fresnica material into an S-strkey stored in the upstream keyring
- [ ] Add an upstream application-level exact-envelope signer backend; keep Account source and expected Signer identity separate
- [ ] Add returned-envelope validation so a backend may add valid signature material but may not mutate the approved transaction
- [ ] Implement the Fresnica backend without exposing passphrase, mnemonic, secret, Reveal or raw `WalletUnlockKey` to MCP
- [ ] Execute the one-Payment Testnet acceptance and mutation suite before expanding operation scope
- [ ] Keep the current operation-type `AgentCapability` dormant; replace or remove it rather than promoting a parallel policy engine

## Historical Mobile binding

- [x] `bindings/mobile` served as the v0.1 compatibility facade and validation donor
- [x] Native SDK / UniFFI plus framework adapters became the authoritative native integration path
- [x] Legacy Mobile binding source, publisher and active compatibility CI were retired from `main`
- [x] Historical tags, release artifacts and archived migration documentation preserve the old contract

## Quality and governance

- [x] Required CI gate for compatibility, Rust surfaces and RefPython
- [x] Core, SDK, Process Binding, Rust client/CLI/TUI and RefPython test coverage
- [x] Native Android/Apple packaging, React Native adapter and WASM validation workflows
- [x] Main bundle artifact workflow for isolated development recovery
- [ ] Enable repository ruleset/branch protection requiring PRs and `Required CI / validate`; prohibit force-push and branch deletion

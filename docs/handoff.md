# Fresnica Project Handoff

Updated: 2026-09-02

This is the continuation/state document for the shared Fresnica repository. Stable architecture and security rules live in the common contracts; start at the [repository README](../README.md), [`docs/README.md`](README.md), and the [Modern Stellar Core Capability Baseline](development/modern-stellar-core-capability-baseline.md).

Do not treat a handoff SHA as permanent truth. Verify GitHub `main`, CI and release metadata before development.

## 1. Repository boundary

`Fresnica/fresnica` is the shared **Core / SDK / Specification / Reference** repository.

```text
Protocol/Core       slow security and protocol primitives
SDK                 stable cross-language semantic contract
Conformance         shared executable vectors
RefPython           semantic/protocol laboratory
Rust Reference      reusable Application Capability reference
Bindings/Adapters   platform delivery of shared security semantics
Products            Mobile/Terminal/Web/Desktop/Agent in independent repos
```

Current source placement:

```text
core/rust
sdk/*
bindings/*
spec/test-vectors
reference/python
reference/rust-client
```

The Rust CLI and TUI still live under `clients/` only as migration source. The agreed target is one independent `Fresnica/fresnica-terminal` repository containing both; do not split CLI and TUI into separate repositories. The connected GitHub capability cannot create a repository and `Fresnica/fresnica-terminal` does not yet exist, so do not delete the only current terminal source until that target exists and its CI is green.

## 2. Modern Stellar baseline

Fresnica is no longer targeting a Classic-only Core.

Current foundation includes:

- Classic transaction signing with exact XDR + network context;
- separate G-account and C-account identity semantics;
- Protocol-28-ready `stellar-xdr` ahead of Mainnet activation;
- CAP-71 AddressV2-aware Soroban authorization preimage/hash/signature primitives;
- direct G-account Ed25519 auth-entry signing and verification;
- fail-closed C-account/custom/delegated authorization;
- a language-neutral Soroban authorization conformance vector;
- `CoreClientApi` protected/external auth-entry signing and `invalid-authorization` classification;
- SDK API v4 auth-entry protected/passcode signing plus external prepare/apply over the shared conformance vector;
- Process Binding API v2 transport for those SDK v4 Soroban authorization operations, adopted only for the concrete RefPython trusted-host consumer;
- RefPython Soroban simulation/assembly/review plus source-account and detached Classic G-account authorization/signing/submission semantics over official `stellar-sdk`, with reviewed authorization expiry, exact assembled/authorized-object binding, explicit restore handling, and Testnet submit/status reconciliation;
- `reference/rust-client` `RpcGateway` plus Soroban prepare/review/authorize/sign/submit semantics over official Protocol-28 `stellar-rpc-client`, keeping Soroban simulation/state/submission on RPC while reusing the existing Horizon-backed Classic signer/threshold lookup for envelope authorization;
- an opt-in RefPython Ledger Stellar HID provider layered above Core's external Ed25519 prepare/apply boundary, with deterministic SEP-5/APDU/request-binding tests and a successful physical macOS Testnet clear-signing proof using Ledger Stellar app 6.0.3, path `m/44'/148'/0'`, Blind Signing disabled, and transaction `f91abc8bd8af37484bbb0c3c0e933e454df3131bb6b21598715e3af8f2beb4b0`.

The next protocol work should remain **consumer- or provider-driven**: SEP-53 message signing is the next independent Core signing domain; C-account/passkey/smart-account work starts from a concrete provider rather than a speculative universal abstraction.

## 3. Security boundary

Preserve these invariants:

```text
Account identity != Signer capability != Recovery source
reviewed object == signed object
network context is part of signing meaning
unsupported authorization/signature forms fail closed
```

Core must not expose a generic hash-signing oracle merely to make an integration easier.

Ledger hardware evidence reinforces this boundary: hardware transport and derivation-path UX stay above Core; Core only prepares/verifies exact signing meaning.

## 4. Soroban ownership

Core owns deterministic security primitives only. It does not own RPC, `simulateTransaction`, sequence fetching, resource estimation, contract-spec interpretation, smart-account deployment or product approval flows.

RefPython validates product/protocol semantics first. `reference/rust-client` is the Rust reference implementation. Concrete C-account/passkey/smart-account providers should precede generic provider abstractions.

## 5. Agent boundary

Soneso Stellar Agent Wallet remains the preferred Agent execution layer for MCP, policy, approval, nonce/replay, audit, RPC construction and submission. Fresnica remains the protected signer authority.

Do not:

- duplicate Soneso's Agent policy stack;
- adapt Fresnica as a raw-hash signer;
- decrypt Fresnica signer material into the upstream keyring;
- expose Process Binding owner privileges to MCP/remote agents.

The upstream exact-envelope signer seam remains the prerequisite for Fresnica Agent integration.

## 6. CI and repository cleanup state

Repository validation is layered:

```text
Local: portable rustfmt + diff review
Required CI: shared Core/SDK/binding/reference contracts
Path workflows: affected reference/product integration
Merge: squash
Post-merge: Main bundle
```

The repository-level SDK boundary guard lives at `scripts/validate-rust-sdk-boundary.sh`; shared validation no longer depends on a script owned by the CLI product tree.

`Required CI / validate` tests `reference/rust-client` directly when affected and no longer compiles CLI/TUI merely because the Rust reference changed. CLI/TUI retain dedicated workflows until they move to `fresnica-terminal`.

## 7. Immediate next work

1. create `Fresnica/fresnica-terminal`, migrate Rust CLI + TUI together, pin/review the shared Fresnica dependency boundary, and prove independent terminal CI;
2. after that proof, remove `clients/rust-cli`, `clients/rust-tui` and their product workflows from this repository;
3. add SEP-53-aligned message signing as the next independent Core signing domain;
4. add a concrete C-account/passkey/smart-account provider only when a real product flow needs it;
5. continue supply-chain and Backup v1 hardening in parallel;
6. resume Agent integration only when the upstream exact-envelope signing seam exists.

## 8. Known repository administration gaps

- `main` branch protection/ruleset is still disabled.
- Many historical probe/relay/validation branches remain. The current GitHub connector can list but not delete branch refs, so they require a repository-admin cleanup path outside this connector.
- The connected GitHub capability can modify existing repositories but cannot create `Fresnica/fresnica-terminal`.

## 9. Start here next session

1. Verify GitHub `main`, `Required CI` and the newest Main bundle.
2. Restore that Main bundle in isolated execution.
3. Read the architecture/roadmap plus the relevant Core/Capability contract.
4. Keep Core changes surgical and protocol/security-driven.
5. If `Fresnica/fresnica-terminal` exists, finish terminal extraction before adding new terminal product features.

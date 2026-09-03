# Fresnica Project Handoff

Updated: 2026-09-03

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

The native Rust CLI and TUI now live together in [`Fresnica/fresnica-terminal`](https://github.com/Fresnica/fresnica-terminal). This shared repository retains `reference/rust-client` as the reusable Rust Capability reference; terminal product code and product CI no longer live here.

## 2. Modern Stellar baseline

Fresnica is no longer targeting a Classic-only Core.

Current foundation includes:

- Classic transaction signing with exact XDR + network context;
- separate G-account and C-account identity semantics;
- Protocol-28-ready `stellar-xdr` ahead of Mainnet activation;
- CAP-71 AddressV2-aware Soroban authorization preimage/hash/signature primitives;
- direct G-account Ed25519 auth-entry signing and verification;
- final SEP-53 v1.0.0 exact-message signing/verification as a separate domain with cross-language vectors, protected/external Core/SDK paths, Native Binding API 3, and React Native system-auth/passcode challenge signing for Mobile;
- fail-closed C-account/custom/delegated authorization;
- a language-neutral Soroban authorization conformance vector;
- `CoreClientApi` protected/external auth-entry signing and `invalid-authorization` classification;
- SDK API v5 preserves the auth-entry protected/passcode plus external prepare/apply contract and adds the separate SEP-53 protected/passcode + prepare/verify message domain over shared conformance vectors;
- Process Binding API v2 remains the privileged transport for the Soroban authorization operations used by the concrete RefPython trusted-host consumer and now compiles against SDK API v5 without exposing SEP-53 remotely;
- RefPython Soroban simulation/assembly/review plus source-account and detached Classic G-account authorization/signing/submission semantics over official `stellar-sdk`, with reviewed authorization expiry, exact assembled/authorized-object binding, explicit restore handling, and Testnet submit/status reconciliation;
- `reference/rust-client` `RpcGateway` plus Soroban prepare/review/authorize/sign/submit semantics over official Protocol-28 `stellar-rpc-client`, keeping Soroban simulation/state/submission on RPC while reusing the existing Horizon-backed Classic signer/threshold lookup for envelope authorization;
- an opt-in RefPython Ledger Stellar HID provider layered above Core's external Ed25519 prepare/apply boundary, with deterministic SEP-5/APDU/request-binding tests and a successful physical macOS Testnet clear-signing proof using Ledger Stellar app 6.0.3, path `m/44'/148'/0'`, Blind Signing disabled, and transaction `f91abc8bd8af37484bbb0c3c0e933e454df3131bb6b21598715e3af8f2beb4b0`.

The next protocol work remains **consumer- or provider-driven**: Mobile can now consume the SEP-53 message domain for dapp challenges; C-account/passkey/smart-account work starts from a concrete provider rather than a speculative universal abstraction.

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

`Required CI / validate` tests `reference/rust-client` directly when affected and does not compile terminal products. CLI/TUI product CI now lives in `Fresnica/fresnica-terminal`.

## 7. Immediate next work

1. validate the first concrete Mobile dapp challenge/session flow against the SEP-53 Native/React Native contract without moving transport policy into Core;
2. add a concrete C-account/passkey/smart-account provider only when a real product flow needs it;
3. continue supply-chain and Backup v1 hardening in parallel;
4. resume Agent integration only when the upstream exact-envelope signing seam exists.

## 8. Known repository administration gaps

- The default-branch repository ruleset is active and requires PRs plus the stable required CI, signed/linear history, resolved conversations, and no force-push/deletion.
- Many historical probe/relay/validation branches remain. The current GitHub connector can list but not delete branch refs, so they require a repository-admin cleanup path outside this connector.

## 9. Start here next session

1. Verify GitHub `main`, `Required CI` and the newest Main bundle.
2. Restore that Main bundle in isolated execution.
3. Read the architecture/roadmap plus the relevant Core/Capability contract.
4. Keep Core changes surgical and protocol/security-driven.
5. For terminal product changes, work in `Fresnica/fresnica-terminal` and update its pinned `FRESNICA_REV` explicitly when adopting a new shared revision.

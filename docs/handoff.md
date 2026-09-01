# Fresnica Project Handoff

Updated: 2026-09-01

This is the continuation/state document for the shared Fresnica repository. Stable architecture and security rules live in the common contracts; start at the [repository README](../README.md), [`docs/README.md`](README.md), and the [Modern Stellar Core Capability Baseline](development/modern-stellar-core-capability-baseline.md).

Do not treat a handoff SHA as permanent truth. Verify GitHub `main`, CI and release metadata before development.

## 1. Logical project boundaries

The repository is a monorepo, but development should treat these as independently evolving layers:

```text
Protocol/Core       slow security and protocol primitives
SDK                 stable cross-language semantic contract
Conformance         shared executable vectors
RefPython           semantic laboratory
Rust Client         wallet capability reference implementation
Bindings/Adapters   platform delivery
Products            Mobile/Web/Desktop/Agent, developed independently
```

Core should change slowly. SDK adapts Core into stable platform-neutral semantics. RefPython may explore application semantics first. RustClient and independent products may advance in parallel without forcing product logic into Core.

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
- RefPython Soroban simulation/assembly/review plus source-account and detached Classic G-account authorization/signing/submission semantics over official `stellar-sdk`, with reviewed authorization expiry, exact assembled/authorized-object binding, explicit restore handling, and Testnet submit/status reconciliation.

The next protocol work is **RustClient RPC/Soroban capability work using that RefPython evidence**, not new Core authorization primitives.

## 3. Security boundary

Preserve these invariants:

```text
Account identity != Signer capability != Recovery source
reviewed object == signed object
network context is part of signing meaning
unsupported authorization/signature forms fail closed
```

Core must not expose a generic hash-signing oracle merely to make an integration easier.

## 4. Soroban ownership

Core owns deterministic security primitives only. It does not own RPC, `simulateTransaction`, sequence fetching, resource estimation, contract-spec interpretation, smart-account deployment or product approval flows.

RefPython should first validate simulation/assembly/review semantics. RustClient may then implement the reference wallet/network capability. Concrete C-account/passkey/smart-account providers should precede generic provider abstractions.

## 5. Agent boundary

Soneso Stellar Agent Wallet remains the preferred Agent execution layer for MCP, policy, approval, nonce/replay, audit, RPC construction and submission. Fresnica remains the protected signer authority.

Do not:

- duplicate Soneso's Agent policy stack;
- adapt Fresnica as a raw-hash signer;
- decrypt Fresnica signer material into the upstream keyring;
- expose Process Binding owner privileges to MCP/remote agents.

The upstream exact-envelope signer seam remains the prerequisite for Fresnica Agent integration.

## 6. CI workflow

Repository validation is layered:

```text
Local: portable rustfmt + diff review
Draft/focused pushes: Required CI + directly relevant lightweight workflows
Ready/non-draft PR: expensive path-relevant integration/platform matrix
Merge: squash
Post-merge: Main bundle
```

`Required CI / validate` is the stable merge-gate name. It should test affected Rust surfaces and immediate contract dependencies, not compile every downstream client/platform on every push.

## 7. Immediate next work

1. use the completed RefPython Soroban simulation/authorization/submission evidence to add the RustClient RPC/Soroban capability reference;
2. adapt Native/Process/WASM only where a concrete consumer needs the stable SDK v4 Soroban authorization contract;
3. add SEP-53-aligned message signing as the next independent Core signing domain;
4. continue supply-chain and Backup v1 hardening in parallel;
5. resume Agent integration only when the upstream exact-envelope signing seam exists.

## 8. Start here next session

1. Verify GitHub `main`, `Required CI` and the newest Main bundle.
2. Restore that Main bundle in isolated execution.
3. Read the Modern Stellar baseline plus the relevant Core/Capability contract.
4. Use CodebaseMemory for orientation/blast-radius analysis, then verify conclusions against source.
5. Run the portable rustfmt tool before pushing Rust changes.
6. Keep Core changes surgical and protocol/security-driven.

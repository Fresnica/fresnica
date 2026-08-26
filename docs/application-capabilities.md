# Fresnica Application Capabilities

## Status

This document defines the cross-platform capability vocabulary, maturity model and governance rules shared by Fresnica products.

Detailed capability contracts live in [`capabilities/`](capabilities/README.md).
Shared cross-capability value semantics live in [`capabilities/domain-primitives.md`](capabilities/domain-primitives.md).

A Capability Contract standardizes **wallet meaning**, not one implementation, language, SDK, storage engine or UI architecture.

> **Unify semantics, not source code. Unify contracts, not UI.**

## 1. Definition

An **Application Capability** is a reusable wallet/application semantic contract below product UI and Application Flows.

A Capability answers:

> **What does this wallet operation mean, what are its inputs/outputs/invariants, and what behavior may consumers rely on?**

Examples include:

- Account;
- Signer;
- Payment;
- Transaction;
- Trustline;
- SDEX;
- Anchor;
- Signing Coordination.

A Capability is not:

- a screen or user journey;
- a Rust crate requirement;
- a React Native Feature;
- a persistence engine;
- a Horizon/RPC wrapper;
- a new cryptographic Core.

The surrounding architecture and term definitions are in [`architecture.md`](architecture.md).

## 2. Flow vs Capability

Application Flows own **why / when / product sequence / confirmation / UI state**.

Application Capabilities own **stable wallet semantics**.

Example:

| Behavior | Owner |
| --- | --- |
| amount input widget | Send Flow |
| amount precision and asset identity | Payment Capability |
| review shown as screen vs modal | Send Flow |
| review must match exact transaction | Transaction Capability |
| BID/ASK layout | SDEX Flow/UI |
| ManageBuyOffer vs ManageSellOffer semantics | SDEX Capability |
| biometric vs passcode product UX | Flow/platform authorization policy |
| Ed25519 signing meaning | Fresnica SDK/Core |

See [`application-flows.md`](application-flows.md).

## 3. Contract, not implementation

A Capability may have multiple conforming implementations:

```text
Payment Capability Contract
        |
        +--> Rust reference implementation (`clients/rust-client`)
        +--> Mobile implementation (for example Stellar JS SDK + Native SDK)
        +--> Web implementation
        +--> Desktop implementation
```

A platform is not required to compile or embed `fresnica-client` merely to conform.

The current Rust client is:

1. reusable Rust application behavior for CLI/TUI;
2. a reference implementation for many Capability semantics;
3. a source of regression/conformance examples where appropriate.

Its module names, storage layout, raw Horizon types and internal helpers are not automatically cross-platform contract surface.

See [`platform-implementation.md`](platform-implementation.md).

## 4. Capability specification shape

A `Normative` Capability should define only the semantics cross-platform consumers need to rely on.

Where applicable, a detailed contract records:

### Purpose

The wallet/application problem the capability solves.

### Semantic inputs

Inputs independent of widgets, command-line arguments and implementation SDK types.

### Semantic outputs/review

Stable results and review/state objects that Flows may depend on.

### Invariants

Rules that must hold across implementations.

Examples:

- account context is network-scoped;
- Account != Signer;
- full issued-asset identity is `CODE:GISSUER`;
- transaction review corresponds to the exact transaction being signed;
- nonzero SDEX values must not be semantically projected as false zero.

### Errors

Stable semantic categories suitable for Flow/product mapping. A Flow should not need to parse arbitrary Rust strings, JavaScript exceptions or transport error bodies to understand common outcomes.

### State/lifecycle

Only where order affects correctness, for example:

```text
prepare -> review -> confirm -> authorize -> sign -> submit
```

or:

```text
verify SEP-10 challenge -> sign exact challenge -> exchange token
```

### Side effects

Durable/network effects and the state that should be invalidated/refreshed.

### Security boundary

Which values may cross into application/platform code and which remain SDK/Core/native/provider-owned.

### Conformance examples

Fixtures or canonical examples when they prevent semantic drift.

A contract should not prescribe internal class names, folder structure, dependency injection frameworks or UI components.

## 5. Maturity

Maturity describes the **shared specification**, not implementation quality.

### Normative

Stable cross-platform semantics exist and conforming implementations should preserve them.

A Normative capability normally has explicit invariants and enough result/lifecycle/error semantics to detect incompatible behavior.

### Defined

The capability name, purpose and ownership boundary are agreed, but detailed APIs/DTOs remain intentionally platform/product specific.

This is appropriate when multiple real implementations are still needed before freezing common semantics.

### Proposed

A candidate capability or semantic extension under discussion. It must not be treated as a stable shared contract.

A production-quality platform implementation may still exist while its common capability remains Defined.

## 6. Capability catalog

The catalog is shared vocabulary, **not a mandatory product feature checklist**.

| ID | Capability | Maturity | Detailed contract/reference |
| --- | --- | --- | --- |
| `account` | Account | Normative | [Account](capabilities/account.md) |
| `signer` | Signer | Normative | [Signer](capabilities/signer.md) |
| `wallet` | Wallet | Defined | [Wallet](capabilities/wallet.md) |
| `balance` | Balance / Availability | Normative | [Balance / Availability](capabilities/balance.md) |
| `payment` | Payment | Normative | [Payment](capabilities/payment.md) |
| `transaction` | Transaction | Normative | [Transaction](capabilities/transaction.md) |
| `trustline` | Trustline | Normative | [Trustline](capabilities/trustline.md) |
| `history` | History / Activity | Defined | [History / Activity](capabilities/history.md) |
| `contacts` | Contacts / Destination Resolution | Defined | [Contacts / Destination Resolution](capabilities/contacts.md) |
| `sdex` | SDEX | Normative | [SDEX](capabilities/sdex.md) |
| `anchor` | Anchor | Normative | [Anchor](capabilities/anchor.md) |
| `signing` | Signing Coordination | Normative | [Signing Coordination](capabilities/signing-coordination.md) |
| `security` | Application Security | Defined | [Application Security](capabilities/application-security.md) |
| `dapp` | Dapp Interaction | Defined | [Dapp Interaction](capabilities/dapp.md) |
| `external-signer` | Hardware / External Signer Interaction | Defined | [External Signer](capabilities/external-signer.md) |
| `network` | Network / Gateway | Defined | [Network / Gateway](capabilities/network.md) |

The capability index is also available at [`capabilities/README.md`](capabilities/README.md).

### Why some entries are Defined

`Wallet`, `History` and `Contacts` are intentionally not promoted merely because the Rust engineering client already has implementations.

Current evidence shows meaningful platform variation:

- Rust terminal storage uses a compact `WalletRecord`, while Mobile models Account/Signer relationships separately;
- Rust history still exposes Horizon-shaped records, while the Python reference has a richer cache/presentation model;
- terminal contacts use a local file and Classic-address resolver that should not freeze Mobile/Web address-book design.

Stable common semantics can be promoted later from real product implementations.

## 7. What must be shared

Across conforming implementations Fresnica should seek to share:

- capability identity/name;
- domain identities and canonical forms;
- request meaning;
- review/result meaning;
- state transitions where order affects correctness;
- security invariants;
- stable error categories where specified;
- cross-capability relationships;
- conformance fixtures/examples where useful.

For Normative capabilities, equivalent semantic input should have equivalent wallet meaning even when internal SDK calls differ.

## 8. What stays platform-specific

Capabilities do not require standardizing:

- React Native/native/TUI/Web UI;
- navigation and Flow composition;
- Zustand/Redux/Swift state libraries;
- Realm/SQLite/files;
- Horizon/RPC client library choice;
- Stellar Rust SDK vs Stellar JS SDK;
- Rust/Swift/Kotlin/JavaScript/C# implementation language;
- Keychain/Keystore/DPAPI/libsecret implementation;
- biometric/system-auth UI;
- HTTP library;
- cache internals;
- dependency injection framework;
- folder/class/function naming.

Do not introduce cross-platform abstractions merely to make implementations look structurally identical.

## 9. Capability evolution

Capability specifications evolve from real implementation experience.

Any platform may propose a new capability or a contract upgrade when it discovers stable cross-platform semantics.

A proposal should explain:

1. the concrete product/implementation need;
2. why the behavior is cross-platform rather than one mechanism;
3. the proposed capability/contract change;
4. compatibility impact;
5. security impact;
6. examples/tests/fixtures where useful.

The governing rule is:

> **Promote stable semantics into the shared specification; keep platform mechanisms local.**

Do not promote a platform implementation detail merely because one client currently needs it.

Do not block useful platform work merely because a `Defined` capability has not yet standardized its detailed mechanism.

## 10. Conformance

Conformance is semantic, not byte-for-byte implementation identity.

Useful checks include:

- canonical request/result examples;
- stable error-category cases;
- identity and amount/price edge cases;
- transaction review/signing binding;
- watch-only/signer lifecycle cases;
- protocol security fixtures;
- platform adapter tests proving secrets do not cross prohibited boundaries.

Existing cross-language fixtures under [`../spec/test-vectors/`](../spec/test-vectors/) include wallet derivation/protection, transaction signing, SDEX semantics and smart-account provider authorization.

Core cryptographic vectors remain SDK/Core-owned. Application semantic fixtures belong to the relevant Capability contract/reference implementation.

## 11. Relationship to current code

Current Rust modules map to the common vocabulary approximately as:

```text
clients/rust-client::wallet       -> Account / Signer / current terminal Wallet implementations
clients/rust-client::service      -> Account / Balance / History reference accessors
clients/rust-client::payment      -> Payment implementation
clients/rust-client::transaction  -> Transaction + part of Signing Coordination
clients/rust-client::trustline    -> Trustline implementation
clients/rust-client::dex          -> SDEX implementation
clients/rust-client::anchor*      -> Anchor implementation
clients/rust-client::contacts     -> Contacts implementation
```

Existing Rust symbols containing `Service` do not require a mechanical rename. Architecture terminology changes first; source names should change only when there is a maintenance/API benefit.

Likewise, a Mobile project may keep any locally useful directory names. Cross-project discussions and contracts use **Application Capabilities** so `Core` remains unambiguous.

## 12. Compact rule

> **Application Flows define user intent and product sequence. Application Capabilities define reusable wallet semantics. Platform code supplies mechanisms. Fresnica SDK/Core remains authoritative for cryptographic meaning.**

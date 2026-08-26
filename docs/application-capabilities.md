# Fresnica Application Capabilities

## Status

This document defines the cross-platform capability vocabulary, maturity model and governance rules shared by Fresnica products.

Detailed capability contracts live in [`capabilities/`](capabilities/README.md).
Shared cross-capability value semantics live in [`capabilities/domain-primitives.md`](capabilities/domain-primitives.md).
Shared error layering lives in [`capabilities/error-semantics.md`](capabilities/error-semantics.md).

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

Stable semantic categories suitable for Flow/product mapping. A Flow should not need to parse arbitrary Rust strings, JavaScript exceptions or transport error bodies to understand common outcomes. Cross-capability error layering is defined in [`capabilities/error-semantics.md`](capabilities/error-semantics.md).

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

Stable cross-platform semantics exist and conforming implementations **must** preserve them.

A Normative capability normally has explicit invariants and enough result/lifecycle/error semantics to detect incompatible behavior.

### Defined

The capability name, purpose and ownership boundary are agreed, but detailed APIs/DTOs remain intentionally platform/product specific.

This is appropriate when multiple real implementations are still needed before freezing common semantics.

### Proposed

A candidate capability or semantic extension under discussion. It must not be treated as a stable shared contract.

A production-quality platform implementation may still exist while its common capability remains Defined.

### Reference Semantics for Defined capabilities

`Defined` must not erase mature implementation experience. When a real implementation already demonstrates useful behavior, its Capability reference should separate:

- **Agreed boundary** — semantics already shared across products;
- **Reference Semantics** — proven behavior that is a candidate for future promotion;
- **Implementation-specific choices** — mechanisms or product policy that must not be frozen yet.

Reference Semantics should link to the implementation and regression tests that provide the evidence. The implementation does **not** need to live in this repository: `fresnica-mobile`, a future Web/Desktop project or another maintained implementation may contribute links to its commits, tests and design evidence through a documentation PR.

Other platforms may adopt, reject or refine Reference Semantics. Concrete divergence should feed the capability-evolution process rather than creating silent semantic forks.

A reference implementation is therefore part of the specification process without becoming the specification by fiat.

### Reference extensions on Normative capabilities

`Normative` describes the stable scope already shared; it does not freeze all future behavior. A mature Capability may record adjacent, implementation-proven behavior as a clearly labeled **Reference extension (non-normative)**.

Examples include liquidity-pool portfolio projection under Balance, product-facing Anchor next actions and other behavior that is valuable but has not yet accumulated enough independent evidence for promotion. These extensions may be adopted, challenged or refined by other platforms without weakening the existing Normative core.

## 6. Capability catalog

The catalog is shared vocabulary, **not a mandatory product feature checklist**. It currently contains **19 Capabilities: 9 Normative and 10 Defined**.

| ID | Capability | Maturity | Detailed contract/reference |
| --- | --- | --- | --- |
| `account` | Account | Normative | [Account](capabilities/account.md) |
| `signer` | Signer | Normative | [Signer](capabilities/signer.md) |
| `wallet` | Wallet | Defined | [Wallet](capabilities/wallet.md) |
| `backup-restore` | Backup / Restore | Defined | [Backup / Restore](capabilities/backup-restore.md) |
| `balance` | Balance / Availability | Normative | [Balance / Availability](capabilities/balance.md) |
| `asset-discovery` | Asset Discovery / Catalog | Defined | [Asset Discovery / Catalog](capabilities/asset-discovery.md) |
| `payment` | Payment | Normative | [Payment](capabilities/payment.md) |
| `transaction` | Transaction | Normative | [Transaction](capabilities/transaction.md) |
| `trustline` | Trustline | Normative | [Trustline](capabilities/trustline.md) |
| `history` | History / Activity | Defined | [History / Activity](capabilities/history.md) |
| `contacts` | Contacts / Destination Resolution | Defined | [Contacts / Destination Resolution](capabilities/contacts.md) |
| `sdex` | SDEX | Normative | [SDEX](capabilities/sdex.md) |
| `anchor` | Anchor | Normative | [Anchor](capabilities/anchor.md) |
| `ledger-authorization` | Ledger Authorization | Defined | [Ledger Authorization](capabilities/ledger-authorization.md) |
| `signing` | Signing Coordination | Normative | [Signing Coordination](capabilities/signing-coordination.md) |
| `security` | Application Security | Defined | [Application Security](capabilities/application-security.md) |
| `dapp` | Dapp Interaction | Defined | [Dapp Interaction](capabilities/dapp.md) |
| `external-signer` | Hardware / External Signer Interaction | Defined | [External Signer](capabilities/external-signer.md) |
| `network` | Network / Gateway | Defined | [Network / Gateway](capabilities/network.md) |

The capability index is also available at [`capabilities/README.md`](capabilities/README.md).

### Why some entries are Defined

A `Defined` label means the capability boundary is useful now but the common contract still needs implementation evidence. It does not mean the capability is unimportant or unimplemented.

Current reasons include:

- **Wallet** — terminal references use compact wallet records while Mobile is expected to model Account/Signer relationships more independently;
- **Backup / Restore** — Rust/RefPython share a useful encrypted terminal v1 format, but future portable backup must protect/revalidate a richer Account/Signer/Recovery graph rather than freeze that record shape;
- **Asset Discovery / Catalog** — RefPython has a useful cache-first multi-source catalog, but provider choice, recommendation metadata, entry DTOs and ranking policy still need independent platform evidence;
- **History** — the Python reference has a mature raw-cache/activity model, while the Rust client still exposes more provider-shaped records and no cross-platform Activity DTO is frozen;
- **Contacts** — destination precedence semantics are promising, but address types, name normalization, sync and storage vary by product;
- **Application Security** — Apple/Android Native SDK system-auth behavior provides strong Reference Semantics, but application-level product contracts still need Mobile/Desktop evidence;
- **Dapp Interaction** — no stable Fresnica request/session/result model exists yet;
- **Ledger Authorization** — the local-signer vs on-ledger authorization boundary is fixed, while a reusable multisig/threshold evaluator still needs real implementation evidence;
- **External Signer** — the provider-neutral security boundary is defined, but no concrete hardware provider implementation is mature enough to contribute additional shared semantics;
- **Network / Gateway** — network identity rules are stable, while Horizon/RPC/provider result models are still evolving.

Stable common semantics can be promoted later from real product implementations and conformance evidence.

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

Any Fresnica implementation may propose a new capability, add Reference Semantics, record a meaningful divergence or propose a contract upgrade. The implementation may live in this repository or in a separate product repository such as `fresnica-mobile`.

The preferred contribution path is a documentation PR against the shared Capability contract. Source code does not need to be copied into this repository; the PR should point to durable implementation evidence.

### Evidence contribution

A useful implementation-evidence PR should record, where applicable:

1. platform/product and implementation repository/commit or PR;
2. which existing semantics the implementation adopted;
3. concrete behavior that differed and why;
4. regression tests, fixtures or real-platform validation that support the behavior;
5. which parts are product/platform mechanism and should stay local;
6. any candidate semantic change that other Fresnica products could rely on.

Evidence may update a `Defined` capability without changing its maturity. This is expected: collecting good Reference Semantics is itself progress.

### Contract-change proposal

A proposal to change shared semantics should additionally explain:

1. the concrete product/implementation need;
2. why the behavior is cross-platform rather than one mechanism;
3. the proposed capability/contract change;
4. compatibility impact;
5. security impact;
6. examples/tests/fixtures where useful.

### Promotion rule

Promotion to `Normative` should be evidence-driven rather than implementation-count-driven, but one implementation's internal shape is normally insufficient by itself. Strong promotion evidence includes:

- convergence of materially different platform implementations;
- independent implementation of the same Reference Semantics;
- protocol/security invariants that necessarily require identical wallet meaning;
- cross-language fixtures demonstrating compatible behavior.

When feasible, a promotion should add or update conformance examples so later implementations can detect semantic drift.

### Normative compatibility and versioning

Reference Semantics and non-normative extensions may evolve normally as implementation evidence accumulates. Clarifications that do not change wallet meaning and corrections that restore the already-intended contract may also update a Normative document in place.

The first change that **intentionally changes previously Normative wallet meaning** must be explicit: introduce or advance the relevant contract/conformance-vector version, document compatibility/migration impact, and avoid silently redefining the old behavior in prose. This keeps a documentation PR from creating an undetectable semantic fork across released Fresnica products.

The governing rule is:

> **Promote stable semantics into the shared specification; keep platform mechanisms local.**

Do not promote a platform implementation detail merely because one client currently needs it.

Do not block useful platform work merely because a `Defined` capability has not yet standardized its detailed mechanism. A platform may implement first, document the result, and then seek to mature the shared contract.

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
clients/rust-client::storage      -> terminal Backup / Restore v1 reference implementation
clients/rust-client::service      -> Account / Balance / History reference accessors
clients/rust-client::payment      -> Payment implementation
clients/rust-client::transaction  -> Transaction + part of Signing Coordination
clients/rust-client::trustline    -> Trustline implementation
clients/rust-client::dex          -> SDEX implementation
clients/rust-client::anchor*      -> Anchor implementation
clients/rust-client::contacts     -> Contacts implementation
reference/python/fresnica/asset_catalog.py -> Asset Discovery / Catalog reference implementation
```

Existing Rust symbols containing `Service` do not require a mechanical rename. Architecture terminology changes first; source names should change only when there is a maintenance/API benefit.

Likewise, a Mobile project may keep any locally useful directory names. Cross-project discussions and contracts use **Application Capabilities** so `Core` remains unambiguous.

## 12. Compact rule

> **Application Flows define user intent and product sequence. Application Capabilities define reusable wallet semantics. Platform code supplies mechanisms. Fresnica SDK/Core remains authoritative for cryptographic meaning.**

# Fresnica Application Capabilities

## Status

This document defines the cross-platform application semantics shared by Fresnica clients.

It is a **capability contract**, not a requirement to share one implementation, one language, one SDK, one storage engine, or one UI architecture.

The canonical term is:

> **Application Capabilities**

In ordinary discussion, **Capabilities** is sufficient.

The old Rust-side term `Service` and the Fresnica Mobile term `Core` may still appear in existing code and older documents, but they should not be used as the cross-platform architectural name going forward. The product layer is called **Application Flows**; Mobile may continue to organize its Flow implementations as local `Feature`s.

---

## 1. Why this layer exists

Fresnica has multiple products and runtimes:

- Rust CLI;
- Rust TUI;
- Fresnica Mobile;
- future Web clients;
- future Desktop clients;
- other future integrations.

Their UI, platform APIs, persistence, network libraries and language ecosystems may differ substantially.

Their **wallet semantics should not**.

For example, every implementation may render a payment review differently, but it should agree on:

- what a payment request means;
- when `CreateAccount` is required instead of `Payment`;
- how amount precision is interpreted;
- how reserve/liability preflight works;
- what watch-only means;
- which signer is eligible;
- what is reviewed before signing;
- what a successful, pending or failed submission means;
- which stable error category is returned.

Application Capabilities define those common semantics.

---

## 2. Canonical architecture terms

Fresnica uses the following terms consistently.

### 2.1 Application Flow

An **Application Flow** is a user goal plus product orchestration. In ordinary discussion, **Flow** is sufficient.

Examples:

- onboarding;
- account management;
- send;
- portfolio;
- history;
- trustlines;
- SDEX trading;
- anchor deposit/withdraw;
- security settings;
- future dApp interaction/approval.

A Flow owns product intent, interaction sequence, confirmation boundaries, transient UI state and product-facing outcome mapping.

A Flow answers:

> **Why is this operation happening, when should it happen, and how should the product guide the user?**

Flows are free to differ between TUI, Mobile, Web and Desktop.

`Feature` is a platform-local organization term. In Fresnica Mobile, a Feature naturally implements one or more Application Flows. It is not the cross-project architectural name.

See [Application Flows](application-flows.md).

### 2.2 Application Capability

An **Application Capability** is a reusable wallet/application semantic contract below product UI.

A **Capability Contract** is the shared specification. A **Capability Implementation** is one concrete platform/language realization of that contract.

Examples:

- Payment;
- Transaction;
- Trustline;
- SDEX;
- Anchor;
- Account/Signer lifecycle;
- Balance/Availability;
- History;
- Signing coordination.

A Capability answers:

> **What does this wallet operation mean, what are its inputs/outputs/invariants, and what behavior must consumers be able to rely on?**

A Capability does **not** imply a particular implementation.

### 2.3 Fresnica Core

**Fresnica Core** means the Rust cryptographic/security authority.

It owns semantics such as:

- secret and mnemonic validation/derivation;
- signer identity derivation and verification;
- protected signer envelopes;
- KDF/encryption semantics;
- transaction hashing/signing;
- signature verification;
- stable crypto/security errors.

The term `Core` should not be reused for the general Mobile application layer.

### 2.4 Native / Universal SDK

The Fresnica SDK exposes Fresnica Core capabilities to consumers through stable platform-neutral/native contracts.

It is not the whole Application Capability layer.

### 2.5 Infrastructure / Ports

Infrastructure provides mechanisms required by Capabilities or Flows, for example:

- Horizon/RPC/network transport;
- Realm/SQLite/file persistence;
- Keychain/Keystore/DPAPI/libsecret;
- system authentication;
- notifications;
- deep links;
- browser or WalletConnect transport.

Infrastructure implementations are platform-specific unless a separate contract explicitly standardizes their semantics.

### 2.6 Presentation / UI / UX

Presentation is entirely product-owned.

TUI, Mobile, Web and Desktop may use different:

- screens;
- navigation;
- forms;
- confirmation flows;
- tables/charts;
- animation;
- biometric prompts;
- error copy;
- interaction patterns.

The Capability contract must not prescribe presentation unless presentation changes the semantic meaning of an operation.

---

## 3. Layering model

```text
Product / App
     |
     v
Application Flows
why / when / product sequence / UI state
     |
     v
Application Capabilities
wallet semantics / contracts / reusable application behavior
     |
     +----------------+----------------+----------------+
     |                |                |                |
     v                v                v                v
Fresnica SDK      Stellar SDK      Repository       Platform Ports
/ Rust Core       / Gateway        / Cache          / Infrastructure
```

The important rule is:

> **Unify wallet semantics and Application Capability contracts. Do not require one platform implementation and do not unify UI/UX.**

---

## 4. Contract, not implementation

A Capability definition may have multiple conforming implementations.

For example:

```text
Payment Capability
        |
        +--> Rust reference implementation (`fresnica-client`)
        |
        +--> Mobile implementation (Stellar JS SDK + Fresnica Native SDK)
        |
        +--> Web/WASM implementation
        |
        +--> Desktop implementation
```

All are valid if they preserve the same normative semantics.

Therefore:

- TUI may call the Rust implementation directly;
- CLI may call the Rust implementation directly;
- Mobile may use Stellar JS SDK plus Mobile-owned orchestration;
- Web may use a WASM/native or JavaScript implementation;
- Desktop may use Rust, Swift, Kotlin, C#, JavaScript or another implementation;
- no platform is required to compile or embed `fresnica-client` merely to conform.

The current Rust `fresnica-client` crate should be treated as:

1. a reusable Rust implementation of many Application Capabilities;
2. a reference implementation for capability semantics;
3. a source of conformance fixtures and regression behavior where appropriate.

It is **not** the definition of the architecture itself.

---

## 5. Capability specification shape

A normative Capability should define only what cross-platform consumers need to rely on.

Where applicable, its specification should contain:

### Name

A stable capability name, for example `Payment` or `Anchor`.

### Purpose

What wallet/application problem it solves.

### Inputs

Stable semantic inputs, independent of UI widgets and implementation libraries.

### Outputs

Stable semantic results and review/state objects.

### Invariants

Rules that must hold across implementations.

Examples:

- full Stellar asset identity is authoritative;
- watch-only has no local signing capability;
- nonzero prices must not be rendered semantically as zero;
- reviewed transaction meaning must correspond to the exact transaction to be signed.

### Errors

Stable error categories that product layers can map into local UX.

A Flow should not need to parse Horizon error strings, JavaScript exceptions or Rust implementation details.

### State / lifecycle

Only when ordering or state transitions affect correctness.

Examples:

```text
prepare -> review -> confirm -> authorize -> sign -> submit
```

or:

```text
prepare SEP-10 challenge -> sign exact challenge -> exchange token
```

### Side effects

What durable/network state may change and what should be invalidated or refreshed.

### Security boundary

Which values may cross the capability boundary and which must remain in Fresnica Core/native/platform secure layers.

### Conformance examples

Test vectors, fixtures or canonical examples when useful.

A specification should not prescribe internal class names, folder structure, framework hooks or implementation-specific helper APIs.

---

## 6. Capability maturity

Not every capability needs the same level of standardization on day one.

Fresnica uses three maturity levels.

### Normative

The capability has stable cross-platform semantics that implementations should conform to.

The specification normally defines inputs, outputs, invariants, errors and meaningful lifecycle rules.

### Defined

The capability name, purpose and ownership boundary are agreed, but implementations are intentionally platform/product specific and no detailed common semantic API is required yet.

This is appropriate when premature abstraction would be worse than independent implementation.

### Proposed

A candidate capability or semantic extension under discussion.

It must not be treated as a stable cross-platform contract.

Maturity is about the **specification**, not implementation quality.

A platform may have a production-quality implementation of a `Defined` capability without Fresnica having standardized its detailed API.

---

## 7. Initial capability catalog

This catalog is the starting cross-platform vocabulary. It can evolve through the governance rule below.

Canonical IDs are documentation/conformance identifiers; they do not require implementations to expose functions or classes with the same names.

| Capability ID | Name | Maturity |
| --- | --- | --- |
| `account` | Account | Normative |
| `signer` | Signer | Normative |
| `wallet` | Wallet | Normative |
| `balance` | Balance / Availability | Normative |
| `payment` | Payment | Normative |
| `transaction` | Transaction | Normative |
| `trustline` | Trustline | Normative |
| `history` | History / Activity | Normative |
| `contacts` | Contacts / Destination Resolution | Normative |
| `sdex` | SDEX | Normative |
| `anchor` | Anchor | Normative |
| `signing` | Signing Coordination | Normative |
| `security` | Application Security | Defined |
| `dapp` | Dapp Interaction | Defined |
| `external-signer` | Hardware / External Signer Interaction | Defined |
| `network` | Network / Gateway | Defined |

A product is **not required to implement every catalog entry**. A capability may exist in Mobile and not yet exist in TUI/Web/Desktop. The catalog standardizes the name and semantics when that capability is present; it is not a feature checklist.

### 7.1 Account — Normative

Defines account identity and account-facing lifecycle semantics.

Includes:

- classic `G...` account identity;
- contract/account identities where applicable;
- watch-only account semantics;
- attach compatible signer material;
- detach local signer capability without deleting account identity;
- account/network identity scope.

It must preserve:

> Account identity != Signer capability.

### 7.2 Signer — Normative

Defines signer identity/capability semantics independent of product UI.

Includes:

- protected software signer;
- external/hardware signer identity;
- signer/account relationship;
- identity verification;
- signing eligibility inputs/results;
- watch-only behavior.

Cryptographic meaning remains Fresnica Core-authoritative.

### 7.3 Wallet — Normative

Defines the wallet/product aggregate used to organize accounts, signers, network and product metadata.

It must not collapse account identity and signer identity into one cryptographic object.

### 7.4 Balance / Availability — Normative

Defines normalized account balances and spendable/available semantics.

Includes reserve/liability-aware interpretation where applicable.

Presentation formatting remains product-owned.

### 7.5 Payment — Normative

Defines payment preparation and review semantics.

Includes where applicable:

- destination resolution;
- native/issued asset identity;
- amount precision;
- destination existence;
- `CreateAccount` vs `Payment`;
- reserve/liability/fee preflight;
- trustline-related transfer constraints;
- memo semantics;
- immutable review meaning;
- watch-only/signing requirements;
- submission result semantics.

Each UI decides how the payment flow is presented and confirmed.

### 7.6 Transaction — Normative

Defines the shared transaction lifecycle and result model.

Conceptually:

```text
intent
  -> prepare
  -> immutable review
  -> user confirmation
  -> current authorization resolution
  -> signing
  -> submission
  -> normalized result
```

Implementations may use different Stellar SDKs and network transports.

### 7.7 Trustline — Normative

Defines add/change/remove trustline semantics.

Includes:

- full asset identity;
- limit semantics;
- removal preconditions;
- balance/liability checks;
- review and transaction result semantics.

### 7.8 History / Activity — Normative

Defines normalized wallet history/activity semantics and network/account scope.

Storage/cache mechanisms remain implementation-specific.

### 7.9 Contacts / Destination Resolution — Normative

Defines reusable contact/destination resolution semantics used by product Flows.

Storage implementation and contact UX remain product/platform-owned.

### 7.10 SDEX — Normative

Defines Stellar DEX semantics including:

- BUY/SELL intent;
- ManageBuyOffer vs ManageSellOffer direction preservation;
- create/update/cancel offer semantics;
- open offers;
- order book;
- full asset identity;
- exact price ratio semantics;
- bid/ask amount interpretation;
- recent trades/fills/candles where exposed;
- stable review/result/error behavior.

Presentation is explicitly not standardized.

A TUI table, Mobile Xaman-style market screen and Desktop trading layout may all conform to the same SDEX Capability.

### 7.11 Anchor — Normative

Defines wallet-facing Stellar anchor/SEP semantics below UI.

Current common scope includes:

- SEP-1 discovery;
- Classic SEP-10 challenge/session semantics;
- SEP-6;
- SEP-24;
- transaction status;
- common SEP-12 customer status/update handoff.

SEP-specific protocol rules belong here rather than in individual Flows/screens.

Anchor interactive/KYC UX remains Flow-owned.

SEP-45 contract-account authentication remains a separate capability boundary until its execution semantics are deliberately specified.

### 7.12 Signing Coordination — Normative

Defines the application-level authorization/signing sequence shared by transaction-producing Flows.

A Flow decides **that** signing is needed.

Signing coordination decides **which authorized local/external signer capability is used and when authorization is required**, then invokes the platform authorization capability/port.

The platform remains free to implement biometric, passcode, hardware-confirmation or other authorization UX according to its policy. Fresnica SDK/Core decides the cryptographic signing meaning.

No Flow may invent its own incompatible passcode-vs-biometric signing semantics.

### 7.13 Application Security — Defined

Defines the reusable application boundary needed by security-related Flows, for example:

- passcode verification/re-protection coordination;
- system-auth availability/enrollment/removal capabilities;
- application lock/session authorization signals;
- secure cleanup/recovery coordination inputs/results.

Detailed implementation remains platform-specific because secure storage and system-auth mechanisms differ. The **Security Settings Flow** owns screens, user confirmation and product policy.

Fresnica Core cryptographic primitives are not owned by this capability.

### 7.14 Dapp Interaction — Defined

Reserved common capability name:

> **Dapp Interaction**

Purpose:

> Allow a Fresnica product to receive, review, authorize and respond to an external application request involving wallet identity, signing or transaction execution.

The cross-platform specification currently standardizes **only this name, purpose and security boundary**.

Each platform may independently implement its product/runtime mechanism, for example:

- in-app browser;
- deep-link request;
- WalletConnect-style transport;
- extension bridge;
- desktop IPC/browser bridge;
- another future protocol.

A platform implementation must still reuse the normative Account/Signer/Transaction/Signing Coordination semantics rather than creating alternate cryptographic rules.

If multiple implementations reveal stable common request/result semantics, Dapp Interaction may later be promoted to `Normative`.

### 7.15 Hardware / External Signer Interaction — Defined

Defines the common product boundary for invoking an external signer provider while preserving Fresnica signer/security semantics.

Transport details such as USB/HID/BLE/vendor SDK are platform/provider-specific.

### 7.16 Network / Gateway — Defined

Defines the application need for Stellar network state, submission and protocol transport.

Horizon, RPC, Portfolio APIs, proxy policy, retry/cache strategy and client construction are implementation-specific unless a narrower capability contract standardizes a semantic result.

---

## 8. What must be unified

Across conforming implementations, Fresnica should seek to unify:

- capability names and purpose;
- domain identities and canonical forms;
- request meaning;
- result/review meaning;
- state transitions where order affects correctness;
- security invariants;
- stable error categories;
- cross-capability relationships;
- compatibility/conformance fixtures where practical.

For mature capabilities, equivalent semantic input should produce equivalent semantic interpretation even when the implementation language and SDK differ.

---

## 9. What must not be forced into one implementation

Application Capabilities do **not** require standardizing:

- React Native vs native vs TUI vs web UI;
- navigation;
- Zustand/Redux/Swift state mechanisms;
- Realm/SQLite/files;
- Horizon/RPC client library choice;
- Stellar Rust SDK vs Stellar JS SDK;
- Swift/Kotlin/JavaScript/Rust implementation language;
- Keychain/Keystore/DPAPI/libsecret implementation;
- system-auth UI;
- HTTP library;
- cache internals;
- dependency injection framework;
- folder layout;
- class/function naming internal to one implementation.

Do not introduce cross-platform abstractions merely to make implementations look structurally identical.

---

## 10. Flow / Capability boundary example: Send

All products may expose a `Send` Application Flow. A platform may package that Flow inside a local Feature/module/screen architecture.

The Flow may own:

```text
recipient form
asset selector
amount input
screen validation
confirmation UX
review presentation
success/failure presentation
navigation
local draft state
refresh intent
```

The Payment/Transaction Capabilities own common semantics such as:

```text
asset identity
amount interpretation
CreateAccount vs Payment
reserve/liability/fee preflight
memo meaning
immutable transaction review
watch-only/signing requirements
normalized submission result
stable capability errors
```

Possible implementations:

```text
Rust TUI
  Send UI
     -> Rust Payment/Transaction capability implementation

Mobile
  Send Feature -> Send Flow
     -> Mobile Payment/Transaction capability implementation
        using Stellar JS SDK + Fresnica Native SDK

Web
  Send Flow
     -> Web capability implementation
        using JavaScript/WASM as appropriate
```

The user experience differs. The wallet meaning does not.

---

## 11. Capability evolution and governance

Capability specifications are expected to evolve from real implementation experience.

Any platform implementation may discover a missing or better semantic contract and propose an upgrade.

Examples:

- Mobile develops Dapp Interaction and discovers a stable request-review-response model;
- Desktop develops hardware signing and discovers a provider-neutral state machine;
- Web develops a better transaction error taxonomy;
- Rust reference implementation finds an invariant required to prevent semantic ambiguity.

The proposal should explain:

1. the concrete product/implementation need;
2. whether the behavior is truly cross-platform;
3. the proposed capability name or contract change;
4. compatibility impact;
5. security implications;
6. examples/tests/fixtures when appropriate.

The governing rule is:

> **Promote stable semantics into the shared specification; keep platform mechanisms local.**

Do not promote a platform-specific implementation detail merely because one client currently needs it.

Do not block a useful platform feature merely because the common specification has not yet standardized its detailed mechanism.

A `Defined` capability exists precisely to allow independent implementation before common semantics are mature.

---

## 12. Conformance philosophy

Where useful, Fresnica should maintain conformance examples or fixtures for `Normative` capabilities.

The goal is not byte-for-byte implementation identity.

The goal is semantic agreement.

Examples:

```text
same Payment request
     |
     +--> Rust implementation
     +--> Mobile implementation
     +--> Web implementation

expected:
- same asset identity
- same amount meaning
- same operation choice
- same review semantics
- same relevant error category
```

For cryptographic/security operations, canonical fixtures remain owned by Fresnica Core/SDK specifications.

For application semantics, fixtures may live with the Application Capability specification/reference implementation.

---

## 13. Relationship to current Fresnica code

Current Rust work maps naturally into this terminology.

Examples:

```text
clients/rust-client::payment      -> Payment Capability implementation
clients/rust-client::transaction  -> Transaction Capability implementation
clients/rust-client::trustline    -> Trustline Capability implementation
clients/rust-client::dex          -> SDEX Capability implementation
clients/rust-client::anchor       -> Anchor Capability implementation
```

Existing Rust type/function names containing `Service` do not need an immediate mechanical rename.

Architecture terminology changes first. Code should only be renamed when there is a concrete maintenance benefit and the change can be done without compatibility churn.

Likewise, Fresnica Mobile may keep a local `core/` folder if useful internally, but cross-project documentation should call the shared semantic layer **Application Capabilities** to avoid confusing it with Fresnica Rust Core.

---

## 14. Cross-platform rule

The compact rule for all Fresnica products is:

> **Application Flows define user intent and product sequence. Application Capabilities define reusable wallet semantics. Platform code provides mechanisms. Fresnica Core/SDK remains the authority for cryptographic meaning.**

And the implementation rule is:

> **Unify semantics, not source code. Unify contracts, not UI. Let each platform implement what fits its runtime, then feed mature common semantics back into the specification.**

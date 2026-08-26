# Fresnica Architecture

## Purpose

This document defines the top-level architecture shared by Fresnica products. Detailed semantics live in the linked contracts; this file is the map, not a duplicate specification.

## Canonical vocabulary

Fresnica uses four architectural terms across projects:

### Application Flow

A user goal and its product orchestration.

Examples: Send, Manage Accounts, Trade, Anchor Deposit, Dapp Approval.

A Flow owns **why**, **when**, product sequence, confirmation points, transient UI state and product-facing outcomes.

A platform may organize Flows however it likes. In `fresnica-mobile`, a `Feature` is the natural code/product unit that implements one or more Application Flows. Rust/Cargo code should avoid using `Feature` as the cross-platform architecture term because `feature` already has a technical meaning there.

See [Application Flows](application-flows.md).

### Application Capability

A reusable wallet/application semantic contract below product UI.

Examples: Account, Signer, Payment, Transaction, Trustline, SDEX, Anchor and Signing Coordination.

A Capability defines **what an operation means**: stable identities, inputs, outputs, invariants, lifecycle and errors. It does not require a shared source implementation.

See [Application Capabilities](application-capabilities.md).

### Fresnica Core

The Rust cryptographic/security authority.

Core owns cryptographic meaning, signer identity derivation/verification, protected signer envelopes, transaction hashing/signing and stable crypto/security errors. `Core` is reserved for this meaning in cross-project documentation.

See [Core Security Boundary](core-security-boundary.md).

### Infrastructure / Port

A platform/runtime mechanism used by a Flow or Capability: Horizon/RPC, a Stellar SDK, Realm/SQLite, Keychain/Keystore, system auth, notifications, deep links, browser transports and similar facilities.

Mechanisms are platform-specific unless a separate contract standardizes their semantic result.

## Layering

```text
Product / App
     |
     v
Application Flows
user intent / product sequence / UI state
     |
     v
Application Capabilities
shared wallet semantics / stable contracts
     |
     +-------------------+--------------------+--------------------+
     |                   |                    |                    |
     v                   v                    v                    v
Fresnica SDK/Core    Stellar SDK/Gateway  Repositories        Platform ports
security authority   chain/protocol mech. durable state       OS/runtime mech.
```

Dependency direction should remain downward. A Flow may compose multiple Capabilities. A Capability may use Core/SDK and narrow infrastructure ports. UI code must not become a crypto or protocol authority.

## Shared semantics, independent implementations

The architecture standardizes behavior rather than source code.

```text
Payment Capability Contract
        |
        +--> Rust implementation (`clients/rust-client`)
        +--> Mobile implementation (for example Stellar JS SDK + Native SDK)
        +--> Web implementation
        +--> Desktop implementation
```

A platform is free to choose its implementation language, Stellar SDK, storage engine and UI framework. A conforming implementation preserves the normative capability semantics and Core security invariants.

The current Rust `fresnica-client` is a reusable Rust implementation and reference implementation for many Capabilities. It is not mandatory runtime code for every Fresnica product.

See [Platform Implementation](platform-implementation.md).

## Product ownership

UI/UX is intentionally not standardized.

For example, the same Send semantics may appear as:

```text
CLI      command -> textual review -> confirmation
TUI      form/panel -> review -> confirmation
Mobile   screens -> review -> system-auth/passcode UX
Web      page/modal -> browser-appropriate authorization UX
Desktop  native/professional workflow
```

The shared contract is underneath those experiences.

## Security authority

The architecture must preserve these non-negotiable distinctions:

```text
Account identity != Signer capability != Recovery source
System Auth != Fresnica passcode
Product authorization != cryptographic meaning
```

Watch-only is an account with no applicable local signer capability. Attaching signer/recovery material must verify identity inside Core/SDK before durable state changes. Reveal/Export is a higher-privilege boundary than routine signing.

See [Core Security Boundary](core-security-boundary.md).

## Specification evolution

Application Capability contracts evolve from real platform work. Mobile, Web, Desktop or Rust implementations may propose new common semantics when they discover a stable cross-platform requirement.

The rule is:

> **Promote stable semantics into shared contracts; keep platform mechanisms local.**

A product does not need to implement every defined Capability, and a new platform-specific capability does not need to wait for a fully normative contract before useful implementation work begins.

# Fresnica Platform Implementation Contract

## Status

This document defines what Fresnica platforms may implement independently and what they must keep semantically compatible.

It is intentionally implementation-neutral.

## 1. Principle

> **Unify semantics, not source code. Unify contracts, not UI.**

A conforming Fresnica product may implement Application Capabilities using the libraries and runtime architecture appropriate to that platform.

Examples:

```text
Rust CLI/TUI
  Application Flows
      -> `fresnica-client` Capability implementation
      -> `fresnica-sdk` / Rust Core

Mobile
  Feature-first Application Flows
      -> Mobile Capability implementations
      -> Stellar JS SDK / Mobile gateways / repositories
      -> Fresnica Native SDK for Core-owned security operations

Web
  Web Flows
      -> JavaScript/WASM Capability implementations
      -> browser/network infrastructure
      -> Fresnica WASM/Core security surface where applicable

Desktop
  Desktop Flows
      -> Rust/Swift/Kotlin/other reviewed implementation
      -> platform secure storage and network mechanisms
      -> Fresnica SDK/Core security surface
```

No product is required to link the Rust `fresnica-client` crate merely to be Fresnica-compatible.

## 2. What a conforming Capability implementation must preserve

For every `Normative` Capability it implements, a platform should preserve the shared contract's:

- capability identity/name;
- domain identities and canonical forms;
- semantic input meaning;
- output/review meaning;
- invariants;
- lifecycle ordering when correctness depends on order;
- security boundary;
- stable error categories where specified;
- cross-capability relationships;
- conformance fixtures/examples where available.

Equivalent semantic requests should have equivalent wallet meaning even when the internal SDK calls are different.

## 3. What platforms may choose independently

A platform may independently choose:

- UI framework and navigation;
- Flow/Feature directory structure;
- Stellar Rust SDK vs Stellar JS SDK vs another suitable SDK;
- Horizon/RPC/Portfolio transport library;
- HTTP stack;
- persistence engine and cache layout;
- dependency injection mechanism;
- state library;
- secure-storage implementation;
- biometric/system-auth mechanism;
- browser/deep-link/WalletConnect/provider transport;
- language-specific class/function names.

These choices are mechanisms, not shared wallet semantics.

## 4. Fresnica SDK/Core dependency rule

When an operation is security/cryptography-authoritative in Fresnica Core, a platform must call the appropriate SDK/Core operation rather than recreating it in application code.

Examples include:

- mnemonic/secret derivation and identity verification;
- protected signer envelopes;
- re-protection;
- transaction hashing/signing;
- signature verification;
- other operations explicitly assigned to Core by the security contract.

A platform Stellar SDK may still build transactions, query network state or implement protocol transport where the Capability contract permits it.

## 5. Rust reference implementation

`clients/rust-client` is the current reference implementation for many Application Capabilities used by CLI/TUI.

Its roles are:

1. reusable Rust application behavior;
2. executable reference for capability semantics;
3. source of regression/conformance cases where useful.

Its internal Rust APIs, class/module names and storage/network implementation are not automatically part of the cross-platform contract.

Do not copy implementation accidents into the specification solely because the Rust reference currently does them that way.

## 6. Mobile implementation

Mobile may implement a normative Capability in TypeScript/JavaScript using the Stellar JS SDK plus Mobile-owned repositories/gateways, while delegating Core-owned security operations to the Fresnica Native SDK.

For example:

```text
Mobile Send Feature
      |
      v
Send Flow
      |
      +--> Mobile Payment Capability implementation
      |      -> Stellar JS SDK / Stellar Gateway
      |
      +--> Mobile Signing Coordination
             -> platform System Auth policy
             -> Fresnica Native SDK / Core
```

This is conforming if Payment/Transaction/Signer semantics match the shared contracts.

Mobile must not be forced to mirror Rust module structure merely for visual symmetry.

## 7. Defined capabilities and platform innovation

A `Defined` Capability deliberately leaves detailed implementation open.

Example: `Dapp Interaction` currently standardizes the name, purpose and security boundary, not one universal transport API.

Therefore Mobile may implement WalletConnect/deep-link/in-app-browser behavior while Web uses an extension/browser bridge and Desktop uses another mechanism.

Once real implementations reveal stable common semantics, any platform may propose promotion or extension of the shared contract.

## 8. Capability evolution

A platform proposal to change the shared contract should identify:

1. the concrete implementation/product need;
2. the stable semantic behavior discovered;
3. why it belongs across platforms rather than in one mechanism;
4. compatibility impact;
5. security impact;
6. examples/tests/fixtures when practical.

The acceptance rule is:

> **Promote stable semantics into the specification; keep platform mechanisms local.**

A mature platform implementation can therefore lead the common specification rather than waiting for the Rust reference implementation to invent every capability first.

## 9. Conformance

Conformance should be semantic, not byte-for-byte implementation identity.

Useful tests include:

- canonical request/result examples;
- stable error-category cases;
- identity and amount/price edge cases;
- transaction review/signing binding;
- watch-only/signer lifecycle cases;
- protocol security fixtures;
- platform adapter tests proving secret material does not cross prohibited boundaries.

Core cryptographic vectors remain Core/SDK-owned. Application behavior vectors belong to the relevant Capability contract/reference implementation.

## 10. Product capability matrix

A product does not need to implement every Capability.

A platform should explicitly record which capabilities it supports and their implementation/conformance status, for example:

```text
Payment             implemented / normative-conformant
SDEX                implemented / normative-conformant
Anchor              partial
Dapp Interaction    implemented / defined capability
External Signer     not implemented
```

Absence is acceptable. Semantic divergence under the same capability name should be treated as a compatibility issue.

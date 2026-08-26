# Fresnica Documentation

This directory is organized around **shared contracts first, implementation detail second**.

## Start here

Every Fresnica product should begin with these five common documents, in order:

1. [Architecture](architecture.md) - shared layering and canonical vocabulary.
2. [Application Flows](application-flows.md) - product intent, sequencing and UI/UX ownership.
3. [Application Capabilities](application-capabilities.md) - cross-platform wallet semantics, maturity and governance.
4. [Core Security Boundary](core-security-boundary.md) - security invariants implementations must not redefine.
5. [Platform Implementation](platform-implementation.md) - how runtimes may implement the contracts independently.

Compact model:

```text
Application Flow
  user goal / product sequence / UI state
        |
        v
Application Capabilities
  shared wallet semantics
        |
        +----------------------+----------------------+------------------+
        |                      |                      |                  |
        v                      v                      v                  v
Fresnica SDK / Core       Stellar SDK/Gateway   Repositories       Platform ports
crypto/security authority chain mechanisms       durable state      OS/runtime mechanisms
```

> **Flows are product-specific. Capabilities are semantically shared. Fresnica Core is authoritative for cryptographic meaning.**

## Detailed references

### Application Capabilities

See [`capabilities/README.md`](capabilities/README.md) for the capability matrix and detailed contracts. Shared supporting vocabulary is defined by [Domain Primitives](capabilities/domain-primitives.md) and [Error Semantics](capabilities/error-semantics.md).

Normative contracts currently include Account, Signer, Balance / Availability, Payment, Transaction, Trustline, SDEX, Anchor and Signing Coordination. The catalog also contains Defined capabilities including Backup / Restore and Ledger Authorization; Defined capabilities deliberately leave more implementation freedom while their cross-platform semantics mature.

### Fresnica Core

See [`core/README.md`](core/README.md) for Core client/security/signer/protection/reveal details.

The short cross-platform authority remains [Core Security Boundary](core-security-boundary.md).

### Platforms

See [`platforms/README.md`](platforms/README.md):

- [Mobile](platforms/mobile/README.md)
- [Desktop](platforms/desktop/README.md)
- [Web](platforms/web/README.md)
- [Terminal engineering clients](platforms/terminal/README.md)

Platform references describe implementation choices, packaging and UX integration. They do not redefine shared Capability semantics.

### SDK

See [`sdk/README.md`](sdk/README.md) for Native/Universal SDK packaging and release references.

### Development / validation

See [`development/README.md`](development/README.md) for local setup, Testnet workflows and validation guides.

### Decisions / archive

- [`decisions/README.md`](decisions/README.md) - historical architecture decisions.
- [`archive/README.md`](archive/README.md) - legacy terminology/history only.

## Project state

- [Roadmap](roadmap.md)
- [Tasks](tasks.md)
- [Current handoff](handoff.md)

These are state/continuation documents, not permanent architecture contracts. They may age faster than the five common documents.

## Mobile handoff

For a Fresnica Mobile developer, the recommended reading sequence is:

```text
this README
  -> five common contracts
  -> platforms/mobile/README.md
  -> independent fresnica-mobile Feature-first design
```

The mapping is:

```text
Mobile Feature
    implements
Application Flow
    consumes
Application Capabilities
```

Mobile may implement Capabilities with Stellar JS SDK + Fresnica Native SDK + Mobile-owned repositories/code. It is not required to link the Rust `fresnica-client` implementation.

## Terminology rule

Historical code/documents may still use `Service` for what is now an **Application Capability**, or use Mobile `Core` for an application capability layer.

Do not use those as new cross-project architecture terms. `Core` is reserved for Fresnica Rust Core/security authority.

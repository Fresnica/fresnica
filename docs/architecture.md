# Fresnica Architecture

## Purpose

This document defines the top-level architecture shared by Fresnica products. Detailed semantics live in the linked contracts; this file is the map, not a duplicate specification.

## Canonical vocabulary

Fresnica uses these architectural terms across projects.

### Application Flow

A user goal and its product orchestration.

Examples: Send, Manage Accounts, Trade, Anchor Deposit, Dapp Approval.

A Flow owns **why**, **when**, product sequence, confirmation points, transient UI state and product-facing outcomes.

A platform may organize Flows however it likes. In `fresnica-mobile`, a `Feature` is the natural code/product unit that implements one or more Application Flows. Rust/Cargo code should avoid using `Feature` as the cross-platform architecture term because `feature` already has a technical meaning there.

See [Application Flows](application-flows.md).

### Application Capability

A reusable wallet/application semantic contract below product UI.

Examples: Account, Signer, Payment, Transaction, Trustline, SDEX, Anchor and Signing Coordination.

A Capability defines **what an operation means**: stable identities, inputs, outputs, invariants, lifecycle and errors. The contract remains authoritative even when multiple products share one implementation.

See [Application Capabilities](application-capabilities.md).

### Fresnica Application Client

The reusable first-party native implementation of Application Capabilities.

CLI/TUI link it directly. Mobile should consume it through a separate Application Binding/FFI once the current Rust reference has clean repository and async/provider boundaries. Desktop may link it directly or through an appropriate native binding.

The Client owns application semantics and consumes SDK/Core plus narrow infrastructure ports. It is not a cryptographic authority and it must not expose provider response shapes as product contracts.

See [Shared Application Client Boundary](decisions/shared-application-client.md).

### Fresnica SDK / Core

The Rust cryptographic/security authority and its platform-neutral SDK boundary.

Core owns cryptographic meaning, signer identity derivation/verification, protected signer envelopes, transaction hashing/signing and stable crypto/security errors. `Core` is reserved for this meaning in cross-project documentation.

The SDK exposes reviewed security/application-security operations over Core. Neither SDK nor Core owns Horizon/RPC URLs, application persistence, UI state, or provider selection.

See [Core Security Boundary](core-security-boundary.md).

### Infrastructure / Port

A mechanism consumed below Application Client or Flow boundaries: DataProvider adapters, repositories, platform storage, system authentication, notifications, deep links, browser transports and similar facilities.

Provider and persistence mechanisms are not shared wallet semantics merely because the Rust Client currently has one implementation of them.

## Layering

The target first-party native shape is:

```text
CLI / TUI --------------------------\
Mobile -> Application Binding / FFI ---> Application Client
Desktop ----------------------------/          |
                                                +--> SDK -> Core
                                                |
                                                +--> DataProvider ports
                                                |      +--> Horizon
                                                |      +--> RPC
                                                |      +--> future data/index providers
                                                |
                                                +--> Repository ports
                                                       +--> terminal filesystem
                                                       +--> mobile/native persistence
```

Product/Flow code remains above the Client. Dependency direction remains downward.

The existing Native SDK/UniFFI security binding is separate from the future Application Binding:

```text
Mobile platform security helpers
        |
        v
Native SDK binding -> Fresnica SDK -> Core
```

Do not add networking or general application persistence to the Native SDK security ABI merely to avoid creating a distinct Application Binding.

## Shared implementation and semantic contracts

Fresnica now prefers a shared Rust Application Client for first-party native products when the runtime can reasonably support it. This reduces duplicated Account/Payment/Trustline/SDEX/network behavior and centralizes provider migrations.

The semantic contracts remain more stable than any Rust-internal API:

```text
Application Capability Contract
        |
        +--> shared Rust Application Client (native first-party default)
        +--> independent implementation only where runtime/platform needs justify it
```

Sharing source does not make every Rust struct, module name or storage/network implementation normative. Web/browser products may continue to implement the same Capability contracts independently when embedding the native Client is unsuitable.

The current crate still lives at `reference/rust-client` because its infrastructure boundaries are not fully promoted yet. Before Mobile adopts it as a runtime dependency, Terminal-specific filesystem ownership and the synchronous Classic vs asynchronous RPC seam must be extracted deliberately.

See [Platform Implementation](platform-implementation.md).

## Client infrastructure boundaries

### DataProvider

Application capabilities consume semantic ledger/data operations. Horizon, RPC and future data/index services are adapters below that semantic boundary.

Network identity and provider endpoint are distinct. Provider families may migrate independently. There is no product-wide `provider=horizon|rpc` switch.

Raw Horizon/RPC response shapes must terminate below public Client DTOs. Account and Balance are the first normalized read-model slices; History/Activity must not be frozen prematurely around Horizon operation JSON.

### Repository

The reusable Client must not encode `~/.fresnica` or another Terminal filesystem layout as a universal application contract.

Current filesystem `WalletStorage` is a concrete Terminal-era adapter. Before Mobile Application FFI adoption, persistence ownership must be expressed behind an application repository boundary so Mobile/native storage can satisfy the same semantics without copying the Client.

### Async/runtime

The current Classic Client API is synchronous while RPC/Soroban evidence is asynchronous. A shared Mobile-capable Client must establish an explicit async application boundary rather than hiding provider calls behind an internal `block_on`.

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

Product/Flow code owns presentation, navigation, confirmation timing, transient state, product/session policy and OS lifecycle. The shared Client owns reusable wallet/application semantics underneath those experiences.

## Security authority

The architecture must preserve these non-negotiable distinctions:

```text
Account identity != Signer capability != Recovery source
System Auth != Fresnica passcode
Product authorization != cryptographic meaning
Application Client != SDK/Core security authority
```

Watch-only is an account with no applicable local signer capability. Attaching signer/recovery material must verify identity inside Core/SDK before durable state changes. Reveal/Export is a higher-privilege boundary than routine signing.

OS authentication remains client/platform-owned. The existing Native SDK security helpers may protect/release Core credentials, but Application Client sharing does not move biometric/Keychain/Keystore policy into Core.

See [Core Security Boundary](core-security-boundary.md).

## Specification evolution

Application Capability contracts still evolve from real platform work. A shared Rust implementation should feed proven semantics back into those contracts rather than silently making implementation accidents normative.

The governing rule is:

> **Share first-party native application behavior where useful; keep semantic contracts authoritative; keep platform mechanisms and security authority in their proper layers.**

A product does not need to implement every defined Capability. Independent implementations remain valid when a runtime cannot reasonably consume the shared Client, but first-party native products should not duplicate a suitable shared Client capability without a concrete platform reason.

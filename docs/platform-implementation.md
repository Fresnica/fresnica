# Fresnica Platform Implementation Contract

## Status

This document defines the default runtime ownership for first-party Fresnica products and the semantic compatibility rule for platforms that require an independent implementation.

## 1. Principle

> **Share first-party native Application Client behavior; keep Capability contracts authoritative; keep UI and platform mechanisms local.**

Fresnica originally standardized semantics while allowing every platform to implement Application Capabilities independently. That remains a valid fallback for runtimes that cannot reasonably consume the shared Client, but it is no longer the default for first-party native products.

The target is:

```text
Rust terminal product
  Application Flows
      -> shared Fresnica Application Client
      -> SDK / Core + DataProvider / Repository ports

Mobile
  Feature-first Application Flows
      -> Application Binding / FFI
      -> shared Fresnica Application Client
      -> SDK / Core + DataProvider / Repository ports

Desktop
  Product Flows
      -> shared Fresnica Application Client directly or through a native binding

Web / unsupported runtime
  Product Flows
      -> conforming Capability implementation
      -> browser/runtime infrastructure
      -> Fresnica security surface where applicable
```

See [Shared Application Client Boundary](decisions/shared-application-client.md).

## 2. Semantic authority

For every `Normative` Capability it implements, a product must preserve the shared contract's:

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

The shared Rust Client is an implementation of these contracts, not a replacement for them. A Rust-internal class/module/type is not automatically normative simply because multiple native products reuse it.

## 3. First-party native default

CLI/TUI, Mobile and future native Desktop products should reuse a suitable shared Client capability rather than maintaining a second implementation of the same chain/application semantics.

A first-party native product may diverge only for a concrete platform reason, such as:

- the Client cannot run in the target runtime;
- a platform API requires a materially different mechanism;
- the shared capability is not yet mature enough for that product;
- the independent implementation is being used deliberately as conformance/evidence work.

Such divergence should be explicit and conformance-tested rather than becoming an accidental permanent fork.

## 4. What products still choose independently

The shared Client does not standardize:

- UI framework and navigation;
- Flow/Feature directory structure;
- screen state management;
- system-auth UI and OS lifecycle;
- secure-storage implementation mechanics;
- notifications and deep links;
- browser/WalletConnect/provider transport that belongs to a product Flow;
- packaging and application update mechanisms.

Below the Client, concrete infrastructure adapters may also differ while satisfying the same semantic port:

- filesystem vs SQLite/Realm/native repository;
- Horizon vs RPC vs future data/index provider for a capability family;
- HTTP/runtime implementation;
- retry/backoff policy when it does not change shared transaction truth semantics.

## 5. SDK/Core dependency rule

When an operation is security/cryptography-authoritative in Fresnica Core, the Client or product must call the appropriate SDK/Core operation rather than recreate it in application code.

Examples include:

- mnemonic/secret derivation and identity verification;
- protected signer envelopes;
- re-protection;
- transaction hashing/signing;
- signature verification;
- other operations explicitly assigned to Core by the security contract.

The Application Client may depend on SDK/Core. SDK/Core must not depend on the Application Client, DataProvider endpoints or repositories.

The existing Native SDK binding remains a security binding over `fresnica-sdk`; it is not the Mobile Application Client binding.

## 6. Rust Application Client

`reference/rust-client` is the current reusable Rust implementation for many Application Capabilities used by CLI/TUI. Its Cargo package is already `fresnica-client`, but its source placement still reflects its origin as a reference implementation.

Its target roles are:

1. shared first-party native Application Capability implementation;
2. executable reference for capability semantics;
3. owner of provider-neutral application DTOs and orchestration;
4. source of regression/conformance cases where useful.

Before promoting it as a Mobile runtime dependency, remove current implementation accidents from its public construction boundary:

- filesystem `WalletStorage(home)` must become a concrete repository adapter rather than the universal persistence contract;
- provider/runtime ownership must support the proven asynchronous RPC path without internal `block_on`;
- raw provider response shapes must terminate below public Client APIs;
- endpoint configuration must remain separate from cryptographic network identity.

Account and Balance typed read models are the first provider-normalization slices. History/Activity is intentionally not treated as a simple follow-on because it requires stable index/history semantics.

## 7. Mobile target

Mobile should not copy the Rust module tree into React Native/TypeScript. It should consume a separate versioned **Application Binding / FFI** whose DTOs and operations expose suitable Client capabilities.

```text
Mobile Feature / Flow
        |
        v
Application Binding / FFI
        |
        v
Fresnica Application Client
   |                  |
   v                  v
SDK / Core       DataProvider / Repository
```

The existing `FresnicaSdkApi` / Native SDK package retains its security role:

```text
Mobile platform security helper
        |
        v
Native SDK binding -> SDK -> Core
```

Application FFI and Native SDK security FFI may ship in the same product distribution, but they require separate authority and versioning. Do not add Account/Balance/network/persistence APIs to `FresnicaSdkApi` merely because that binding already exists.

Mobile remains authoritative for:

- screens/navigation and Feature orchestration;
- dapp peer/origin/session policy not defined by the protocol;
- system-auth UI and platform lifecycle;
- platform secure-storage mechanics;
- collection of user configuration such as custom provider endpoints.

The Client remains authoritative for reusable application semantics exposed through the Application Binding.

## 8. DataProvider rule

Data providers are Client infrastructure, not product business APIs.

```text
Application Capability
       |
       v
provider-neutral Client model
       |
       v
DataProvider adapter
   +--> Horizon
   +--> RPC
   +--> future data/index provider
```

Provider families may migrate independently. A product should not choose one global `provider=horizon|rpc` mode.

A provider endpoint is not a Stellar network identity. The selected network/passphrase remains security-significant even when a custom Horizon/RPC endpoint is supplied.

## 9. Repository rule

Application persistence must be expressed independently from Terminal's current filesystem layout before Mobile uses the shared Client.

The semantic repository boundary should preserve wallet/application invariants while allowing concrete native persistence appropriate to the product. Database/atomicity/encryption mechanisms remain adapter concerns unless a shared contract explicitly assigns semantic meaning to them.

## 10. Async/runtime rule

The proven RPC/Soroban path is asynchronous while current Classic `FresnicaClient` methods are synchronous.

The shared Client migration must resolve this at the application boundary. It must not:

- hide an async provider behind an internal `block_on`;
- make Terminal/Mobile consume `RpcGateway` directly;
- create a speculative universal provider ORM merely to make all transports look identical.

## 11. Independent implementations

A runtime that cannot reasonably consume the shared Client may implement the same Capability independently using an appropriate Stellar SDK/runtime.

This remains especially relevant to Web/browser environments. Such implementations must preserve the same semantic/security contracts and should use conformance fixtures from the shared Capability work.

Independent implementation is therefore a supported portability mechanism, not the default reason for first-party native products to duplicate application logic.

## 12. Capability evolution

A `Defined` Capability may still be implemented before the common contract is mature. Real product evidence should feed back into the shared specification.

The acceptance rule is:

> **Promote proven semantic behavior into Capability contracts and the shared Client where appropriate; keep mechanisms local.**

A product-specific Flow or transport does not automatically belong in the Client.

## 13. Conformance

Useful tests include:

- canonical request/result examples;
- stable error-category cases;
- identity and amount/price edge cases;
- transaction review/signing binding;
- watch-only/signer lifecycle cases;
- provider-normalization parity tests;
- platform adapter tests proving secret material does not cross prohibited boundaries;
- cross-product tests proving Application FFI preserves Client DTO/semantic meaning.

Core cryptographic vectors remain Core/SDK-owned. Application behavior vectors belong to the relevant Capability contract/shared Client.

## 14. Product capability matrix

A product does not need to implement every Capability.

A platform should explicitly record both support and runtime ownership, for example:

```text
Payment             shared Client / normative-conformant
SDEX                shared Client / normative-conformant
Anchor              shared Client / partial
Dapp Interaction    product Flow + shared security capabilities
External Signer     not implemented
```

Absence is acceptable. Semantic divergence under the same Capability name is a compatibility issue whether the implementation is shared or independent.

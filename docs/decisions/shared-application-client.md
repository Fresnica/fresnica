# Shared Application Client Boundary

Date: 2026-09-06

Status: **Accepted target architecture; staged migration**

## Decision

First-party native Fresnica products should converge on one reusable Rust **Application Client** for wallet/application capabilities that are not inherently UI- or platform-specific.

The target dependency shape is:

```text
CLI / TUI --------------------------\
Mobile -> Application Binding / FFI ---> Fresnica Application Client
Desktop ----------------------------/          |
                                                +--> Fresnica SDK -> Core
                                                |
                                                +--> DataProvider ports
                                                |      +--> Horizon adapter
                                                |      +--> RPC adapter
                                                |      +--> future data/index providers
                                                |
                                                +--> Repository ports
                                                       +--> terminal filesystem adapter
                                                       +--> mobile/native persistence adapter
```

The existing Native SDK / UniFFI binding remains a separate security boundary:

```text
Mobile platform security helpers -> Native SDK binding -> Fresnica SDK -> Core
```

It must not be expanded into the Application Client merely to reuse an existing FFI package.

## Why

The previous architecture deliberately allowed each platform to implement Application Capabilities independently. That protected platform freedom while the shared contracts were still immature, but it also means Account, Payment, Trustline, SDEX, network/provider migration and related application semantics can be implemented more than once.

The Rust `fresnica-client` has now accumulated real reusable application semantics used by both CLI and TUI. Recent work also demonstrates the provider boundary needed for broader reuse:

- `NetworkProfile` separates Stellar network identity from the current provider endpoint;
- Account live state is normalized into provider-neutral typed data before products consume it;
- Balance state is normalized into provider-neutral typed data before products consume it;
- Classic authorization can normalize both Horizon signer JSON and protocol-native XDR `AccountEntry` into the same semantic model.

This makes a shared native Application Client preferable to maintaining separate first-party Mobile and Terminal implementations of the same wallet semantics.

## Boundaries

### Application Client owns

- reusable Application Capability implementations;
- transaction preparation/review/submission orchestration that is not UI-specific;
- provider-neutral Account, Balance and other justified domain DTOs;
- network profile resolution after product configuration is supplied;
- selection and composition of concrete DataProvider adapters below semantic capability methods;
- repository-facing application semantics once repository ports are extracted.

### SDK / Core own

- cryptographic meaning;
- account/signer identity validation assigned to Core;
- protected signer envelopes;
- transaction hashing/signing and signature verification;
- stable security-domain operations and errors.

The Application Client may consume SDK/Core. SDK/Core must not depend on the Application Client or on network providers.

### Product / Flow layer owns

- UI/UX and navigation;
- confirmation timing and presentation;
- product/session policy;
- dapp peer/origin/session policy where the protocol does not define it;
- OS lifecycle and system-auth UX;
- collection and persistence policy for user-configurable endpoint settings.

### DataProvider adapters own

- Horizon/RPC/provider transport;
- provider request/response shapes;
- provider-specific pagination/retry details;
- normalization inputs required by the Application Client.

Provider JSON/XDR transport accidents must terminate below the public product-facing Client contract. Provider families may migrate independently; there is no global `provider=horizon|rpc` product switch.

### Repository adapters own

- concrete persistence mechanisms such as filesystem, SQLite/Realm, or another native store;
- platform storage lifecycle and atomicity mechanisms.

Repository mechanisms must not redefine wallet/application semantics.

## Existing Native SDK binding remains security-only

`bindings/native` is intentionally a framework-neutral UniFFI wrapper over `fresnica-sdk`. It exposes security-domain operations and native system-auth helpers. It contains no application networking or persistence contract.

Mobile adoption of the shared Application Client therefore requires a **separate versioned Application Binding/FFI surface** rather than adding network/application state to `FresnicaSdkApi` or `NATIVE_BINDING_API_VERSION`.

The two native surfaces may ship together in a product package, but their authority and versioning remain distinct.

## Required migration before Mobile uses the Rust Client

The current `reference/rust-client` is reusable by Terminal but still contains implementation choices that must not become accidental Mobile contracts:

1. `FresnicaClient::new/from_profile` currently constructs a filesystem `WalletStorage` from `home`; extract repository ownership behind an application repository boundary before Mobile adoption.
2. Classic client methods are synchronous while the proven RPC/Soroban path is asynchronous; establish an explicit async application boundary rather than hiding RPC behind `block_on`.
3. Continue terminating provider-shaped read data below the Client API. Account and Balance are the first completed slices; History/Activity must wait for stable indexer/activity semantics rather than freezing Horizon operations as the universal model.
4. Keep endpoint configuration separate from network identity. Products provide resolved configuration; the Client consumes it; provider adapters consume concrete endpoints.
5. Do not move OS authentication into Core or generic Client semantics. Existing platform security helpers remain the authority for system-auth-protected credential release.

## Migration sequence

```text
1. Normalize provider-shaped Client outputs
   Account -> Balance -> only then other justified read models

2. Extract Client infrastructure boundaries
   Repository ports + provider/runtime/async boundary

3. Promote the Rust implementation from reference-only placement
   without making Rust-internal names the cross-platform specification

4. Add a separate Application Binding / FFI
   typed DTOs, explicit versioning, no provider JSON

5. Adopt shared Client capability-by-capability in Mobile
   remove duplicated Mobile chain semantics only after conformance proof

6. Migrate provider families independently
   Horizon -> RPC / index provider where semantics are proven
```

## Compatibility rule

Application Capability contracts remain the semantic source of truth. Sharing one Rust implementation does **not** make every Rust struct/function automatically normative, and it does not force Web or another unsupported runtime to link the Rust Client.

A platform may still implement a Capability independently when the shared Client cannot reasonably run there, but first-party native products should not duplicate an already suitable shared Client capability without a concrete platform reason.

## Non-goals

- one UI or Flow implementation across products;
- folding Client into SDK/Core;
- folding Application FFI into the existing Native SDK security ABI;
- a universal lowest-common-denominator DataProvider ORM;
- a single provider switch for all Stellar data families;
- forcing History/Activity into an RPC model before an index/history contract exists;
- forcing browser/Web products to embed the native Rust runtime.

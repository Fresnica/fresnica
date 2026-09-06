# Fresnica Mobile Documentation Entry

This directory describes how the independent `fresnica-mobile` product consumes Fresnica contracts, the published Native SDK security surface, and the target shared Application Client architecture.

## Read first

Before Mobile-specific implementation documents, read the common contracts in this order:

1. [`../../architecture.md`](../../architecture.md)
2. [`../../application-flows.md`](../../application-flows.md)
3. [`../../application-capabilities.md`](../../application-capabilities.md)
4. [`../../core-security-boundary.md`](../../core-security-boundary.md)
5. [`../../platform-implementation.md`](../../platform-implementation.md)
6. [`../../decisions/shared-application-client.md`](../../decisions/shared-application-client.md)

Then read the independent Mobile project's Feature-first architecture.

The target vocabulary mapping is:

```text
Mobile Feature
    implements one or more
Application Flows
    consume
Application Binding / FFI
    exposes suitable capabilities from
Fresnica Application Client
    consumes
SDK / Core + DataProvider ports + Repository ports
```

A Mobile Feature is a local product/code-organization unit. It is not the cross-platform name for an Application Capability.

## Current published security integration

The existing Native SDK remains the authoritative Mobile **security** integration surface. It wraps `fresnica-sdk` / Core operations for protected signer material, signing and system-auth helpers. It is not the future Application Client binding and must not be widened with Horizon/RPC/application persistence merely to reuse an existing FFI package.

- [SDK usage](sdk-usage.md) - current package/version/consumer baseline.
- [Native bindings](bindings.md) - Native SDK/UniFFI security boundary and compatibility history.
- [Framework adapter](framework-adapter.md) - React Native security-adapter source/binary contract.
- [System authentication](system-auth.md) - Mobile system-auth lifecycle.
- [Security vault mapping](security-vault-contract.md) - detailed Mobile persistence/native security mapping.

## Application Client migration status

A Mobile Application Binding / FFI is a **target**, not a currently published package.

Before Mobile adopts the shared Rust Client, the Client must first remove current Terminal-era implementation assumptions:

- filesystem `WalletStorage(home)` must become repository/adaptor ownership rather than a universal Client contract;
- Classic synchronous network calls and the proven asynchronous RPC/Soroban path need an explicit application runtime boundary;
- provider response shapes must terminate below Client DTOs;
- network identity/passphrase must remain separate from configurable provider endpoints.

Account and Balance are the first provider-neutral read-model slices. History/Activity remains intentionally deferred until stable index/history semantics exist.

Until the Application Binding is available, existing Mobile networking/persistence code remains an implementation reality, but it is no longer the target authority for duplicated first-party Application Capability semantics.

## Migration/reference material

- [Application migration reference](app-migration-pr81-pr84.md)
- [React Native upgrade playbook](react-native-upgrade-playbook.md)
- [Legacy Mobile SDK v0.1.0 release](../../archive/mobile-sdk-v0.1.0.md) - historical compatibility only; not a new-project baseline.

## Mobile ownership

Mobile continues to own:

- screens/navigation/Feature organization;
- Flow/product/session policy;
- concrete Realm/native persistence adapters and migrations;
- platform system-auth/secure-storage integration;
- Dapp/browser/deep-link mechanisms;
- collection/persistence policy for product configuration such as custom provider endpoints;
- runtime/platform adapters required to connect the shared Client to Mobile facilities.

Mobile does **not** own:

- secret/mnemonic cryptographic derivation;
- protected signer envelope meaning;
- transaction hashing/signing semantics;
- alternate signer identity rules;
- provider-neutral Account/Balance/application semantics already supplied by a suitable shared Client capability;
- a separate first-party Horizon/RPC business model once the shared Client supplies that capability;
- cross-platform Capability redefinition without updating the common contract.

Realm, Keychain/Keystore and other platform mechanisms may remain Mobile-specific adapters. Their concrete storage/API choices do not become Application Capability semantics merely because Mobile uses them.

A mature Mobile implementation may still propose upgrades to the common Capability specification. The preferred path is a documentation PR that links to the `fresnica-mobile` implementation commit/tests, records which Reference Semantics were adopted or changed, and proposes only behavior that proved reusable across products. Independent Capability implementations remain valid when the runtime cannot reasonably consume the shared Client, but they are no longer the default for first-party native Mobile work.

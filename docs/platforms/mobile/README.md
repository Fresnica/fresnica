# Fresnica Mobile Documentation Entry

This directory describes how an independent `fresnica-mobile` product consumes Fresnica contracts and the Native SDK.

## Read first

Before Mobile-specific implementation documents, read the common contracts in this order:

1. [`../../architecture.md`](../../architecture.md)
2. [`../../application-flows.md`](../../application-flows.md)
3. [`../../application-capabilities.md`](../../application-capabilities.md)
4. [`../../core-security-boundary.md`](../../core-security-boundary.md)
5. [`../../platform-implementation.md`](../../platform-implementation.md)

Then read the independent Mobile project's Feature-first architecture.

The vocabulary mapping is:

```text
Mobile Feature
    implements one or more
Application Flows
    consume
Application Capabilities
    use
Fresnica Native SDK / Stellar JS SDK / repositories / platform ports
```

A Mobile Feature is a local product/code-organization unit. It is not the cross-platform name for an Application Capability.

## Native SDK integration

- [SDK usage](sdk-usage.md) - current package/version/consumer baseline.
- [Native bindings](bindings.md) - Native SDK/UniFFI boundary and compatibility history.
- [Framework adapter](framework-adapter.md) - React Native adapter source/binary contract.
- [System authentication](system-auth.md) - Mobile system-auth lifecycle.
- [Security vault mapping](security-vault-contract.md) - detailed Mobile persistence/native security mapping.

## Migration/reference material

- [Application migration reference](app-migration-pr81-pr84.md)
- [React Native upgrade playbook](react-native-upgrade-playbook.md)
- [Legacy Mobile SDK v0.1.0 release](../../archive/mobile-sdk-v0.1.0.md) - historical compatibility only; not a new-project baseline.

## Mobile ownership

Mobile owns:

- screens/navigation/Feature organization;
- Flow/product policy;
- Realm/application persistence and migrations;
- Stellar JS SDK/gateway implementation where chosen;
- platform system-auth/secure-storage integration;
- Dapp/browser/deep-link mechanisms;
- Capability implementations that are not directly provided by a suitable native/shared implementation.

Mobile does **not** own:

- secret/mnemonic cryptographic derivation;
- protected signer envelope meaning;
- transaction hashing/signing semantics;
- alternate signer identity rules;
- cross-platform Capability redefinition without updating the common contract.

A mature Mobile implementation may propose upgrades to the common Capability specification.

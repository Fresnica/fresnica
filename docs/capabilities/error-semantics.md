# Error Semantics

This supporting contract defines how Fresnica Application Capabilities preserve error meaning across Core, capability implementations, platform mechanisms and Application Flows.

It does **not** require every language to expose one identical `Error` class or wire DTO.

## Layers

Fresnica errors have four semantic layers:

```text
Fresnica SDK/Core security errors
        ↓
Application Capability semantic outcomes
        ↓
Infrastructure / transport diagnostics
        ↓
Application Flow product presentation
```

### SDK/Core security errors

Stable SDK/Core error categories preserve cryptographic/security meaning such as invalid transaction material, invalid authorization material, identity mismatch or protected-data failure.

A platform adapter must not translate distinct security categories into one opaque native/framework exception before a Capability can reason about them.

### Capability semantic outcomes

A Capability should expose distinctions that affect wallet behavior or safe recovery, for example:

- invalid semantic input;
- missing applicable signer / watch-only source;
- authorization denied or cancelled;
- insufficient balance/reserve/fee capacity;
- deterministic transaction rejection;
- submission outcome uncertain;
- protocol/customer information required;
- capability unavailable or unsupported.

Capability documents may define narrower categories where the distinction is domain-significant.

### Infrastructure / transport diagnostics

HTTP status, provider error bodies, socket failures, Horizon/RPC payloads, native exception strings and retry metadata are diagnostics/mechanism. They may be retained for logging/debugging when safe, but ordinary Flows must not have to parse them to recover stable wallet meaning.

Sensitive values such as secrets, passcodes, unlock keys, SEP authentication tokens and submitted KYC values must not be added to diagnostics merely to make errors more descriptive.

### Flow/product presentation

A Flow translates Capability outcomes into product copy, screen state, retry/cancel actions and telemetry policy.

Presentation text is platform/product-specific. Product translation must not destroy distinctions needed for safe behavior.

## Distinctions that must not be collapsed

At minimum, implementations must not silently collapse:

- **watch-only / no applicable signer** into **authorization denied**;
- **user cancelled/denied authorization** into **invalid credential**;
- **deterministic transaction rejection** into **submission uncertain**;
- **confirmed transaction success followed by refresh failure** into **transaction failure**;
- **identity mismatch** into a generic storage/network error;
- **protocol requires more customer information** into a generic transport failure.

These distinctions affect what a wallet may safely retry, what it should ask the user to do next and whether a transaction may already exist on chain.

## Cross-platform rule

Equivalent semantic failures should remain distinguishable across conforming implementations even when concrete language types differ.

A Rust enum, JavaScript discriminated union, Swift enum or Kotlin sealed type may all conform if the Flow can make the same security/product decision without parsing implementation-specific strings.

## Evolution

New stable error meaning should normally be added to the owning Capability first. Promote a cross-capability category here only when multiple Capabilities or platforms need the same distinction.

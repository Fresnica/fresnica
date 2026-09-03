# Protocol Awareness and Demand-Driven Implementation

Status: **accepted engineering principle**

Date: 2026-09-03

## Decision

Fresnica follows this rule:

> **Protocol-complete awareness; demand-driven implementation.**
>
> Follow the protocol proactively; implement capabilities empirically.

Protocol definitions set the safety boundary. Concrete user and product needs set implementation order.

## What protocol awareness requires

Fresnica Core should proactively track security-significant Stellar protocol changes even when no product currently exposes them. At minimum, Core and the shared specification layer should be able to:

1. recognize relevant new identity, signer, authorization, envelope and preimage forms;
2. preserve their security meaning without silently flattening them into older forms;
3. know when the current implementation cannot safely satisfy them;
4. fail closed rather than signing an incompletely understood object.

Protocol awareness is therefore a safety obligation, not a commitment to immediately implement every new protocol capability.

## What remains demand-driven

Full implementations should normally be justified by concrete evidence from Mobile, RefPython, Terminal, Web, Agent or another real consumer.

Examples:

- SEP-53 was implemented because Mobile dapp authentication needs message signing.
- Hardware transports such as Ledger USB/BLE or Tangem NFC belong to product/provider layers and should reuse Core's exact-object prepare/verify boundaries rather than moving device transport into Core.
- Soroban `signAuthEntry` should be exposed through additional platform bindings when a concrete consumer needs it; its Core semantics need not be redesigned merely to anticipate a future UI.
- Passkey/C-account support should begin with one real provider and Testnet proof before a generic provider abstraction is frozen.
- Hash-X, signed-payload and delegated signer production support should be completed when a real account or product flow needs them, while unsupported protocol forms continue to be recognized and rejected safely.

## Layering consequence

The default decision process is:

```text
upstream protocol change
        |
        v
Does it change signing/security meaning?
        |
      yes
        v
Core/spec must recognize and fail closed safely
        |
        v
Does a real consumer need to exercise it?
        |
      yes
        v
prove the flow in a concrete consumer/provider
        |
        v
promote only stable cross-product semantics into Core/SDK
```

This avoids two failure modes:

- building a protocol museum containing speculative implementations with no product evidence;
- building only visible product features while remaining unable to safely recognize newer protocol semantics.

## Practical priority rule

When deciding what Fresnica should implement next:

1. **Protocol safety work may preempt the backlog** when an upstream change could cause misinterpretation or unsafe signing.
2. **Product capability work is otherwise demand-driven** and ordered by concrete user value.
3. **Core does not grow merely because a provider or platform needs transport, lifecycle, storage or UI code.**
4. **Reference implementations are evidence generators.** RefPython is used to discover protocol/Core semantics; products such as Mobile reveal actual user requirements; only their stable intersection should normally be promoted into shared Core/SDK contracts.

In short: protocol is the guardrail; demand is the steering input.

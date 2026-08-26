# Fresnica Application Flows

## Status

This document defines the cross-platform **Application Flow** boundary.

An Application Flow is a product/user-goal concept. It is not a requirement that all Fresnica products share identical screens, navigation, state libraries or source code.

The canonical short name is **Flow**.

## 1. Definition

> **Application Flow = user intent + product sequence + confirmation boundaries + transient product state + product-facing outcome mapping.**

A Flow answers:

- what is the user trying to accomplish?
- what must happen before the operation may continue?
- which Capabilities are required and in what product-significant order?
- when is explicit user confirmation required?
- what should the product show or do after success, pending state or failure?
- which local/cache views should be refreshed or invalidated?

A Flow does not define the cryptographic meaning of signing or reimplement stable wallet/chain semantics already owned by an Application Capability.

## 2. Relationship to Mobile Features

`fresnica-mobile` uses Feature-first organization. That is compatible with this contract.

```text
Mobile Feature
     |
     +--> implements one or more Application Flows
     |
     +--> consumes Application Capabilities
```

For example:

```text
features/send
     -> Send Flow
     -> Payment + Transaction + Signing Capabilities
```

`Feature` remains a valid Mobile code/product term. **Application Flow** is the cross-project architecture term because other runtimes may organize the same user goal differently and Rust/Cargo already uses `feature` technically.

## 3. Flow responsibilities

A Flow may own:

- screens/views/terminal prompts;
- navigation and product sequence;
- feature/flow-local transient state;
- product validation such as required UI choices;
- confirmation timing;
- mapping capability results/errors into product outcomes;
- cancellation/back behavior;
- cache refresh/invalidation intent;
- platform-specific UX policy that does not change wallet semantics.

Examples of valid Flow-owned differences:

- Mobile may use a review screen and biometric prompt;
- TUI may use a panel and `y/n` confirmation;
- CLI may accept non-secret command arguments and print a textual review;
- Web may use a modal or browser authorization ceremony.

## 4. Flow non-responsibilities

A Flow must not independently define:

- secret/mnemonic derivation;
- KDF/encryption/signature algorithms;
- protected signer envelope parsing;
- raw `WalletUnlockKey` handling;
- account/signer identity rules;
- Stellar operation direction/price semantics;
- SEP protocol validation;
- generic Horizon/RPC client construction;
- durable repository engine details;
- cross-flow signing/authentication rules.

If a behavior is reusable wallet meaning rather than product sequence, it belongs in an Application Capability.

## 5. Flow to Capability rule

A useful placement test is:

```text
Does changing this rule change what the wallet operation means?
  yes -> Application Capability

Does changing this rule only change how/when the product guides the user?
  yes -> Application Flow
```

Examples:

| Behavior | Owner |
| --- | --- |
| Send amount input widget | Flow |
| Amount precision / stroop meaning | Payment Capability |
| Show confirmation as screen vs modal | Flow |
| Immutable review must match exact transaction | Transaction Capability |
| BID/ASK panel layout | Flow/UI |
| ManageBuyOffer vs ManageSellOffer semantics | SDEX Capability |
| Ask for biometric vs passcode according to platform policy | Flow/platform auth coordinator |
| Ed25519 signing meaning | Fresnica Core/SDK |

## 6. Common Flow names

These names are shared vocabulary, not mandatory product checklists:

- Onboarding Flow;
- Account Management Flow;
- Send Flow;
- Trustline Flow;
- Portfolio Flow;
- History Flow;
- SDEX Trading Flow;
- Anchor Deposit Flow;
- Anchor Withdrawal Flow;
- Security Settings Flow;
- Dapp Approval / Interaction Flow;
- Reveal / Export Flow.

A platform may split or combine these when its UX warrants it. The underlying Capabilities remain the semantic compatibility boundary.

## 7. Transaction-producing Flow pattern

A transaction-producing Flow normally composes the shared lifecycle rather than inventing its own signing pipeline:

```text
user intent
   |
validate product input
   |
request capability preparation
   |
receive immutable semantic review
   |
product presents review
   |
user confirms
   |
resolve current authorization/signing capability
   |
shared signing coordination -> Fresnica SDK/Core
   |
submit through capability/gateway
   |
normalize result
   |
refresh/invalidate affected state
```

The Flow owns the product sequence around these boundaries. Capabilities own the stable meaning inside them.

## 8. Example: Send Flow

The Send Flow may own:

```text
recipient/asset/amount form
local draft
screen-level completeness checks
review presentation
confirmation
success/failure navigation
refresh intent
```

It consumes, as needed:

```text
Contacts / Destination Resolution
Balance / Availability
Payment
Transaction
Signing Coordination
```

It does not independently reimplement destination existence semantics, CreateAccount-vs-Payment selection, reserve/liability preflight, memo meaning or signing rules.

## 9. Cross-flow composition

Flows should not depend on another Flow's internal implementation.

Avoid:

```text
Send -> Portfolio internals
Trustline -> Accounts screen internals
Dapp -> Send internals
```

Instead:

- move reusable wallet behavior down into a Capability;
- move cross-flow product orchestration up into the App/application coordinator;
- invalidate shared repository/cache state and let interested Flows refresh themselves.

## 10. Testing

A Flow should be testable at its application boundary without requiring full native/runtime integration for ordinary product rules.

Use injected Capability/Repository/Port implementations to test:

- validation and sequencing;
- confirmation boundaries;
- result/error mapping;
- cancellation;
- refresh/invalidation behavior;
- regressions proving transaction Flows use the shared signing path.

Native/device integration tests remain responsible for proving the real platform adapter and authorization mechanisms.

## 11. Compact rule

> **Flows define why, when and how the product guides the user. Capabilities define what the wallet operation means.**

# Ledger Authorization Capability

Maturity: **Defined**

## Purpose

Ledger Authorization answers a question that is intentionally separate from local signer custody:

> Given the exact prepared Stellar transaction and current ledger state, which account authorization conditions must be satisfied for this transaction?

A Fresnica Signer record means the application can potentially invoke a signer/provider. It does **not** prove that the current ledger authorizes that signer to perform the prepared transaction.

The concrete rules below currently describe Classic account authorization. Contract/smart-account providers have different on-chain authorization data, but share the same separation between local provider availability and actual authorization for the reviewed transaction; they should contribute provider evidence without pretending Classic thresholds apply to `C...` accounts.

## Agreed boundary

For Classic accounts, a conforming implementation must eventually be able to reason about:

- the transaction source account;
- operation-specific source accounts;
- current master weight and low/medium/high thresholds as applicable;
- current ledger signer entries and their weights;
- the actual operation set/preconditions in the prepared transaction;
- which authorization conditions remain unsatisfied.

Authorization must be evaluated against the **actual prepared transaction**, not a generic `canSign(account)` boolean.

## Ledger signer identity is typed

Stellar ledger signer entries are authorization conditions, not automatically Fresnica local/provider Signer records. They may include forms such as:

- Ed25519 public-key signer;
- pre-authorized transaction signer;
- Hash-X signer;
- signed-payload signer.

Only a condition that Fresnica can actually satisfy through an available local/provider capability should be mapped to an invokable signing path. Do not populate the local signer repository merely by copying every ledger signer entry.

## Multi-source transactions

A transaction may contain operations with source accounts different from the transaction source. Authorization resolution must therefore cover every source/threshold requirement represented by the exact prepared transaction.

Checking only the transaction source is insufficient for a general Dapp/multisig implementation.

## Relationship to Signing Coordination

```text
Prepared Transaction
       |
       v
Ledger Authorization
  -> required ledger conditions / weights / thresholds
       |
       v
Signing Coordination
  -> available Fresnica signer/provider capabilities
  -> collect/verify required authorization material
```

When a product claims multisig support, Signing Coordination must not treat the first valid signature as completion if the Ledger Authorization result still requires additional weight/conditions.

## Non-ownership

Ledger Authorization does not own:

- private keys, mnemonics or protected signer envelopes;
- biometric/System Auth prompting;
- provider transport;
- transaction construction/review;
- persistence of local signer secrets;
- `SetOptions` signer/threshold configuration Flows.

Those belong to Signer, Signing Coordination, Transaction, Application Security, provider layers or future account-management Flows.

## Current implementation status

The boundary is Defined because Fresnica already depends on the conceptual distinction (`hasLocalSigner != ledger-authorized-for-this-transaction`), but the Rust/RefPython references do not yet provide one complete reusable multisig/threshold evaluator.

Current single-signer Classic flows should therefore remain explicit about their supported scope instead of presenting that scope as a universal authorization algorithm.

## Promotion criteria

Promote this capability when a real multisig/delegated-signing implementation provides:

- normalized ledger signer/threshold inputs;
- operation-source aware authorization evaluation;
- typed authorization conditions;
- tests for insufficient/sufficient weight and mixed-source transactions;
- clear handoff to Signing Coordination without duplicating key custody.

# Account Capability

Maturity: **Normative**

## Purpose

The Account Capability defines the identity and lifecycle semantics of an account that Fresnica can observe or operate.

It deliberately separates **account identity** from **signer capability**.

## Semantic identity

An account context is network-scoped:

```text
AccountContext = Stellar network + account identity
```

The same address string on different Stellar networks must not share chain-derived balances, history, offers, pending transactions, anchor sessions or caches.

Supported identity families include:

- Classic Stellar account: `G...`;
- contract/account identity: `C...`, where a product supports it.

Identity parsing/validation that has cryptographic meaning must use Fresnica SDK/Core semantics.

## Required semantics

A conforming implementation must preserve:

1. **Account != Signer.** An account is not defined by whichever local signer is currently available.
2. **Watch-only is an account state/capability relationship, not a key type.** A watch-only account has no applicable local signer capability and requires no Fresnica passcode or recovery material for ordinary reads.
3. **Detach preserves identity.** Removing a local signer capability does not delete or change the account identity.
4. **Attach verifies identity before durable mutation.** For a direct Classic master-key attachment, Core/SDK must derive the supplied secret/mnemonic signer identity and compare it with the expected `G...` identity before persistence changes.
5. **Contract identity is not Ed25519 identity.** A `C...` address must not be treated as proof that an arbitrary `S...` secret owns the contract.
6. **Ledger authorization is application/network state.** A signer that differs from the Classic master key may be authorized through Stellar signer/threshold state; that relationship must not be inferred from string equality alone.

## Inputs and outputs

Cross-platform implementations should expose semantic equivalents of:

- network/account identity parsing;
- account existence/state lookup when required by a Flow;
- account record creation/import/watch-only registration;
- account-to-signer relationship inspection;
- attach/detach operations coordinated with the Signer Capability;
- normalized account identity/status suitable for Flows.

The specification does not require one database schema or one `AccountRecord` wire shape.

## Errors

Flows should be able to distinguish at least:

- invalid account identity/input;
- network mismatch;
- identity mismatch during signer attachment;
- unsupported account/signing mode;
- network/account lookup failure.

Core-owned identity errors should preserve stable SDK/Core categories such as `invalid-input` and `identity-mismatch` where applicable.

## Security boundary

Account operations must never require UI code to parse protected signer envelopes or handle raw private signing material.

See also:

- [Signer Capability](signer.md)
- [Core Security Boundary](../core-security-boundary.md)
- [Network / Gateway](network.md)

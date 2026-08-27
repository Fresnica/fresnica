# Shared Application Domain Primitives

Status: **Normative supporting vocabulary** for Application Capability contracts.

This file defines small cross-capability values whose meaning must remain consistent even when implementations use different language types or Stellar SDK objects.

It is not a new Application Capability.

## Network context

Chain-derived state is scoped by Stellar network identity.

At minimum, an implementation must not confuse mainnet and testnet merely because an account/address string is identical.

Where an exact transaction signing context is involved, the Stellar network passphrase is part of the cryptographic transaction domain.

Custom network endpoint configuration is an implementation concern unless a product explicitly defines a shared custom-network profile contract.

## Account identity

Semantic account identity includes:

- account kind where relevant (`Classic` / contract);
- canonical Stellar address;
- network context for chain-derived application state.

A Classic `G...` account may expose its Ed25519 public signer identity. A contract `C...` address must not be reinterpreted as an Ed25519 public key.

Account identity does not imply local signer availability.

A Stellar muxed `M...` address is not a second independent Classic ledger account. It carries the underlying Classic account plus muxed ID. A Capability that supports muxed addresses must preserve that full semantic identity; a Capability that does not support them must reject explicitly. It must never silently strip `M...` to `G...` and continue as if the destination were unchanged.

## Signer identity

A signer identity is the public identity expected to authorize a signing operation or provider result.

For Ed25519 software/external signers this is a Classic `G...` public key identity. Provider metadata is separate from the public signer identity.

Signer identity does not imply account identity or recovery-source identity.

## Recovery Source

A Recovery Source records provenance/capability used to recover or derive wallet authority, such as a user-held mnemonic source or another product-defined recovery mechanism.

It is distinct from:

- Account identity;
- current Signer capability;
- raw recovery secret material.

A Recovery Source record must not be treated as current on-ledger authorization, and its metadata must not require persisting plaintext mnemonic/secret material in ordinary application state.

## Asset identity

Classic asset identity is exactly one of:

```text
XLM
CODE:GISSUER
```

An issued asset is identified by both code and issuer. Code-only equality is never sufficient across account balances, trustlines, SDEX markets or payments.

Issued-asset code bytes/case are part of identity. Product display policy may prefer uppercase, but a Capability must not uppercase/lowercase a protocol-valid issued code for comparison or construction. `USD:G...` and `usd:G...` are distinct identities when both are protocol-valid.

Capability implementations may use native Stellar SDK `Asset` objects internally, but cross-platform semantic DTOs/fixtures should preserve the full identity. If a language SDK convenience constructor would normalize a protocol-valid asset code, the adapter must use an exact construction path or reject explicitly rather than silently change identity. Shared conformance cases are fixed by [`../../spec/test-vectors/asset-identity-v1.json`](../../spec/test-vectors/asset-identity-v1.json).

## Classic asset amount

Classic Stellar asset amounts use seven decimal places of precision.

Cross-language semantic values should be represented in a form that preserves exact base-10 meaning, for example a decimal string or integer stroops.

Rules:

- do not use binary floating point as the authority for transaction amounts;
- do not silently round an input with more than seven decimal places when an exact transaction amount is required;
- preserve zero/nonzero meaning during formatting.

Current test vectors use base-10 strings for human amounts.

## Memo

The current shared Payment/Anchor memo vocabulary is:

- none;
- text, limited by Stellar's encoded 28-byte memo field;
- unsigned 64-bit ID;
- exact 32-byte hash.

`MEMO_RETURN` / return-hash is a distinct Stellar memo type and is outside the current shared Payment/Anchor product scope. A platform SDK being able to construct it does not permit an implementation to silently reinterpret it as ordinary hash memo semantics.

## SDEX pair and price

A user-facing pair is:

```text
BASE / COUNTER
```

Shared SDEX semantics use:

- `amount` = BASE units;
- `price` = COUNTER units per one BASE unit.

Current Fresnica user-entered offer amount and decimal price inputs are limited to at most seven decimal places. That is a Fresnica product/input semantic, not the precision limit of an exact ledger `Price { n, d }`.

When Stellar exposes an exact price fraction, preserve integer `{n,d}` semantics where correctness/projection depends on it; read-side exact ratios must not be rounded back to the user-input precision before semantic calculations.

A positive price below seven-decimal display precision must not be represented semantically as exact zero.

See [SDEX](sdex.md).

## Transaction identity

A transaction hash is meaningful only in its network/signing context. Applications may use the canonical transaction hash for submission reconciliation, pending-write guards, history correlation and diagnostics.

A transport timeout does not invalidate the transaction identity or prove non-submission.

## Opaque secure values

The following values are not general application domain primitives and must remain in their security boundary:

- raw secret seeds;
- mnemonic phrases except explicit import/generation/reveal flows;
- Fresnica app passcodes;
- native unlock keys;
- protected signer envelope internals;
- anchor JWT/session secrets.

Applications may hold opaque protected envelopes or provider tokens only according to their relevant security/session contracts.

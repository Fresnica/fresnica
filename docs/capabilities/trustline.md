# Trustline Capability

Maturity: **Normative**

## Purpose

The Trustline Capability defines add, limit-change and remove semantics for Classic Stellar issued-asset trustlines.

Product UI may call this **Manage Assets**; the chain semantics remain `ChangeTrust`.

## Asset identity

A trustline asset is always full issued-asset identity:

```text
CODE:GISSUER
```

The asset code must be a valid Classic alphanumeric asset code and the issuer must be a Classic `G...` account.

Native XLM is not a trustline asset.

## Operations

### Add

- requires that the trustline does not already exist;
- creates a `ChangeTrust` operation with a positive limit;
- must preflight native reserve and fee capacity for the additional subentry.

The current Fresnica product default for newly created trustlines is the visible marker limit:

```text
708269837873.6765
```

This is a current product policy, not a Stellar protocol constant. Existing user limits must not be rewritten merely to match it.

### Set limit

- requires an existing trustline;
- the new limit must not be below current balance plus buying liabilities;
- must preserve fee/reserve correctness.

### Remove

- requires an existing trustline;
- encodes zero limit/removal semantics;
- must reject removal while balance, selling liabilities or buying liabilities are non-zero.

## Issuer rule

An asset issuer cannot create a trustline to its own issued asset.

## Prepared review

A review should expose at least:

- add / set-limit / remove intent;
- source account;
- full asset identity;
- resulting limit when applicable;
- fee;
- network.

## Signing/submission

Writes use the shared Transaction and Signing Coordination contracts. Watch-only/no-local-signer sources are not executable writes.

## Errors

Stable semantic errors should distinguish invalid asset/input, already-exists/not-found, insufficient reserve/fee capacity, limit-below-commitment, non-zero balance/liabilities on removal, no signer and submission failure.

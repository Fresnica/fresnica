# Trustline Capability

Maturity: **Normative**

## Purpose

The Trustline Capability defines add, limit-change and remove semantics for ordinary Classic Stellar issued-asset trustlines.

Product UI may call this **Manage Assets**; the chain semantics remain `ChangeTrust`. Liquidity-pool-share `ChangeTrustAsset` is outside the current Normative product scope and must not be enabled merely because a platform Stellar SDK exposes it.

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
- requires the issued-asset issuer account to exist for a new ordinary trustline;
- must preflight native reserve and fee capacity for the additional subentry.

When a Fresnica product creates a trustline without an explicit user-supplied limit, the canonical Fresnica limit is:

```text
708269837873.6765
```

This value is a **Normative Fresnica product semantic**, not a Stellar protocol constant. It intentionally creates a recognizable public on-chain default that can support aggregate Fresnica usage measurement. Because other software can choose the same limit, the value is a statistical/product marker rather than cryptographic proof that one specific trustline was created by Fresnica.

A Flow may allow the user to explicitly choose another valid positive limit. Existing user limits must not be rewritten merely to match the canonical default. Implicit trustline creation performed by another Capability, including SDEX receiving-trustline preparation, must use the same canonical limit.

### Set limit

- requires an existing trustline;
- the new limit must not be below current balance plus buying liabilities;
- a nonzero resulting ordinary issued-asset trustline requires the issuer account to still exist;
- must preserve fee/reserve correctness.

### Remove

- requires an existing trustline;
- encodes zero limit/removal semantics;
- must reject removal while balance, selling liabilities or buying liabilities are non-zero;
- must also reject deletion while the trustline is still referenced by a liquidity-pool relationship (`liquidityPoolUseCount != 0`);
- does not require a deleted/orphaned asset's issuer account to be recreated merely to remove the existing zero-commitment trustline.

## Authorization/clawback state

Successful `ChangeTrust` creation does not always mean the asset is immediately free to receive/send. Initial trustline flags depend on issuer account flags: an `AUTH_REQUIRED` issuer can leave the new trustline unauthorized, and an issuer with clawback enabled causes the trustline's clawback flag to be set.

Capabilities/Flows must preserve this ledger state rather than translate every successful add into a generic "asset ready" result. Full authorization and `AUTHORIZED_TO_MAINTAIN_LIABILITIES` have different Payment/SDEX meaning.

At prepare/review time, implementations may expose the issuer-derived expected initial state. That state is a preflight observation, not a substitute for refreshing the resulting trustline after ledger confirmation because issuer flags can change between preparation and execution.

## Issuer rule

An asset issuer cannot create a trustline to its own issued asset.

## Prepared review

A review should expose at least:

- add / set-limit / remove intent;
- source account;
- full asset identity;
- resulting limit when applicable;
- current/expected authorization and clawback state when the trustline will remain after the operation;
- fee;
- network.

## Signing/submission

Writes use the shared Transaction and Signing Coordination contracts. Watch-only/no-local-signer sources are not executable writes.

## Errors

Stable semantic errors should distinguish invalid asset/input, already-exists/not-found, insufficient reserve/fee capacity, limit-below-commitment, non-zero balance/liabilities on removal, no signer and submission failure.

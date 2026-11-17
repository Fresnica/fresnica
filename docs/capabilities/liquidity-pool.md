# Liquidity Pool Capability

Maturity: **Normative**

Current Normative scope: Classic constant-product **pool exit** and optional pool-share trustline cleanup. Deposit semantics are not standardized by this version.

## Purpose

This capability defines how a Fresnica product exits a Stellar Classic liquidity-pool position without inventing platform-specific share, slippage, review or cleanup rules.

Pool-share balances remain distinct from ordinary `CODE:GISSUER` balances. Portfolio projection may use cached pool data for display; transaction preparation may not.

## Pool identity

A position is identified by:

- Stellar network;
- source Classic account;
- exact 32-byte liquidity-pool ID;
- the pool's canonical asset A / asset B ordering.

Asset A / B ordering comes from the live pool parameters. UI ordering must not swap `min_amount_a` and `min_amount_b`.

Pool-share amounts use exact seven-decimal Stellar amount semantics. Binary floating point must not determine shares, outputs or minimums.

## Exit request

An exit request contains:

- `pool_id`;
- `mode`: `exact_shares` or `all`;
- `shares` for `exact_shares`;
- explicit `slippage_bps`;
- `close_pool_share_trustline` (default `false`).

`exact_shares` supports partial or full numeric withdrawal. `all` means all currently withdrawable shares observed during preparation.

A percentage slider is Flow/UI input. It must resolve to an exact share amount before transaction construction.

`all` is not dynamic after preparation. If the position changes, normal Transaction freshness/sequence rules apply and a new preparation/review is required.

## Live withdrawal snapshot

Preparation requires live account state and live pool state. A cached portfolio position is never sufficient to authorize an exit.

The snapshot records at least:

- pool ID and canonical assets A / B;
- reserve A and reserve B;
- total pool shares;
- the source account's withdrawable pool-share balance;
- observed ledger sequence when available;
- snapshot time.

A preparation snapshot may be at most 30 seconds old. Implementations may require fresher data. Local age checks should use monotonic elapsed time when available so wall-clock rollback cannot extend the window.

If live pool/account state cannot be obtained, preparation fails even when cached portfolio data exists.

Snapshot expiry gates preparation only. Once an exact transaction is prepared, its minimum amounts and the common Transaction TimeBounds protect the authorized operation; hardware or multisig signing is not forced into the 30-second snapshot window.

## Expected outputs

Use integer stroops. For withdraw shares `S`, total pool shares `T`, and reserves `RA` / `RB`:

```text
expected_a = floor(S * RA / T)
expected_b = floor(S * RB / T)
```

This matches the protocol's withdrawal direction: reserve outputs are rounded against the withdrawer.

If both expected outputs are zero, reject as `withdrawal_too_small`. If exactly one side rounds to zero, the exit remains valid but review must show that side as zero.

## Slippage protection

`slippage_bps` is explicit:

```text
0 <= slippage_bps <= 1000
100 bps = 1%
```

The Capability has no hidden default. `1000` bps is Fresnica's cross-product safety ceiling, not a Stellar protocol limit.

For each side:

```text
raw_min_i = floor(expected_i * (10000 - slippage_bps) / 10000)

if expected_i > 0:
    min_i = max(1 stroop, raw_min_i)
else:
    min_i = 0
```

The prepared `LiquidityPoolWithdraw` operation uses the exact share amount plus `min_amount_a` and `min_amount_b`.

Slippage protects against movement away from the reviewed pool snapshot. It does not prove that the current pool composition or external market price is fair.

## Preflight

Before presenting an exit as executable, verify from current state:

- pool-share trustline exists;
- requested shares are positive and do not exceed current withdrawable pool shares;
- pool exists, is a supported constant-product pool and has positive total shares;
- current expected outputs can be represented safely;
- ordinary issued reserve assets have the required receiving trustline/capacity unless the source is that asset's issuer;
- native/issued receiving capacity covers the current expected output;
- current XLM can pay the prepared transaction fee;
- the prepared operation count and reserve/sponsorship effects are reflected in review.

Do not reuse Payment's full-authorization rule blindly. Pool exit is an unwind operation; preflight should preserve current protocol withdrawal semantics rather than blocking risk reduction merely because an issued-asset line is not fully authorized for ordinary new receipt. Authorization revocation can also force pool-share redemption, so stale cached state must not be treated as an active position.

Receiving trustlines are not created implicitly by this Capability.

The network remains authoritative at execution. Liquidity changes can still produce `UNDER_MINIMUM`, and receiving state changes can still produce `LINE_FULL`.

## Full exit and pool-share trustline cleanup

With `close_pool_share_trustline=false`, full withdrawal leaves the zero-balance pool-share trustline in place.

Cleanup is explicit. `close_pool_share_trustline=true` is valid only with `mode=all` and prepares one atomic transaction:

```text
LiquidityPoolWithdraw(all resolved shares, min A, min B)
ChangeTrust(pool-share asset, limit = 0)
```

The second operation is pool-share `ChangeTrust`, not ordinary issued-asset Trustline Capability removal. Construct its pool-share `ChangeTrustAsset` from the live canonical pool parameters (asset A, asset B and protocol fee), and verify that those parameters derive the selected `pool_id`. A mismatch fails closed.

A pool-share trustline counts as two account subentries under the current protocol. Closing it releases that footprint, subject to sponsorship ownership. Review must list cleanup separately and include its fee plus current reserve/sponsorship effect. A second confirmation prompt is not required; explicit opt-in plus the immutable transaction review is sufficient.

After cleanup, a future deposit requires creating the pool-share trustline again. Deleting the last pool-share trustline for a pool may also remove the corresponding pool ledger entry; this is protocol state, not a separate user asset loss.

## Review binding

The immutable review exposes at least:

- network and source account;
- pool ID;
- canonical asset A and B identities;
- partial/full intent and exact shares to withdraw;
- snapshot ledger/time;
- expected amount A / B;
- `slippage_bps`;
- exact `min_amount_a` / `min_amount_b`;
- whether the pool-share trustline will be kept or closed;
- the exact operation list;
- fee and Transaction TimeBounds.

Changing shares, pool snapshot, minimums, cleanup choice, fee policy, network or transaction preconditions requires a new prepared review.

## Submission

Signing, submission and uncertain-submission recovery use the existing Transaction and Signing Coordination contracts.

If submission is uncertain, do not prepare and submit a replacement exit until the original transaction identity is reconciled or the common Transaction policy permits retry.

## Errors

Stable categories include:

- `invalid_request`;
- `slippage_out_of_range`;
- `pool_not_found`;
- `pool_state_unavailable`;
- `pool_state_stale`;
- `pool_share_trustline_missing`;
- `insufficient_pool_shares`;
- `receiving_trustline_missing`;
- `receiving_capacity_insufficient`;
- `withdrawal_too_small`;
- `under_minimum`;
- `close_requires_full_exit`;
- `close_precondition_failed`;
- `pool_identity_mismatch`;
- common fee/authorization/signing/submission errors.

Protocol result mapping:

- `LIQUIDITY_POOL_WITHDRAW_MALFORMED` -> `invalid_request`;
- `..._NO_TRUST` -> `pool_share_trustline_missing`;
- `..._UNDERFUNDED` -> `insufficient_pool_shares`;
- `..._LINE_FULL` -> `receiving_capacity_insufficient`;
- `..._UNDER_MINIMUM` -> `under_minimum`.

## Platform boundary

Mobile/Web may implement pool lookup and transaction construction with their platform Stellar SDK/provider. Native SDK/FFI remains the cryptographic/signing boundary and does not gain Horizon liquidity-pool networking merely to centralize this Capability.

## Conformance

Cross-platform cases are in [`../../spec/test-vectors/liquidity-pool-exit-v1.json`](../../spec/test-vectors/liquidity-pool-exit-v1.json).

# Path Payment / Swap Capability

Maturity: **Normative**

## Purpose

This capability defines atomic Classic-asset conversion with Stellar Path Payment operations.

A Path Payment can convert one asset into another while paying another Classic account. A **Swap** is the same semantic operation with the destination account equal to the source account. Swap is not a third ledger operation and must not be implemented as a hidden offer workflow.

This contract covers Classic assets only:

- native XLM;
- issued assets identified as `CODE:GISSUER`.

Soroban token swaps belong to contract/protocol capabilities, not this contract.

## Modes

Two user intents are supported.

### Strict send

The source amount is exact. The destination amount is quoted and protected by a minimum:

```text
fixed:  source amount
quote:  destination amount
limit:  destination_min
ledger: PathPaymentStrictSend
```

### Strict receive

The destination amount is exact. The source amount is quoted and protected by a maximum:

```text
fixed:  destination amount
quote:  source amount
limit:  source_max
ledger: PathPaymentStrictReceive
```

For a Swap:

- strict send means "sell exactly this much";
- strict receive means "buy exactly this much".

## Semantic request

A request contains the semantic equivalents of:

- network;
- source Classic account;
- destination Classic account;
- source asset;
- destination asset;
- mode: `strict_send` or `strict_receive`;
- the fixed-side amount;
- optional transaction memo.

Source and destination assets must differ.

The current shared scope uses Classic `G...` accounts. Muxed-account support must be added explicitly rather than silently demuxed.

A destination must already exist. Path Payment does not inherit Payment Capability's XLM `CreateAccount` behavior.

## Quote

A quote is a short-lived observation of available path liquidity. It is not a rate guarantee.

A quote exposes at least:

- network;
- mode;
- source and destination account;
- source and destination asset;
- quoted source amount;
- quoted destination amount;
- ordered intermediate asset path;
- quote time;
- expiry time;
- observed ledger sequence when the provider exposes one.

Amounts use exact seven-decimal Stellar semantics. Binary floating point must not determine a quoted amount or safety limit.

The `path` contains **intermediate assets only**. Source and destination assets are not repeated in it. Stellar Path Payment permits at most five intermediate assets; longer paths are invalid.

The signed operation commits to this asset path. It does **not** commit to particular offers or liquidity pools inside a hop. A product must not present a quote as if it reserved specific liquidity.

If multiple candidates are returned for the same request, the default ordering is deterministic:

1. strict send: larger destination amount first;
2. strict receive: smaller source amount first;
3. fewer intermediate assets;
4. canonical lexical path identity as the final tie-breaker.

A product may let the user select another candidate, but preparation must use the selected candidate exactly.

## Quote lifetime

A live quote has a maximum lifetime of **30 seconds**:

```text
expires_at <= quoted_at + 30 seconds
```

An implementation may use a shorter lifetime. It must not extend a live quote beyond 30 seconds without producing a new quote.

At or after `expires_at`, preparation fails with `quote_expired`. Re-quoting may change the amounts or asset path and therefore requires a new review.

The prepared transaction TimeBounds must not outlive the quote:

```text
transaction.max_time <= quote.expires_at
```

If authorization cannot complete inside the remaining window, re-quote before asking the user to authorize.

Observed ledger sequence is quote provenance, not a substitute for the time expiry rule.

## Slippage

Slippage is expressed as integer basis points:

```text
0 <= slippage_bps <= 1000
100 bps = 1%
```

The Capability has no silent default. A product Flow may choose a default, but it must pass the selected value into preparation and show it in review.

Safety limits are derived in integer stroops.

For strict send:

```text
destination_min =
  floor(quoted_destination_stroops * (10000 - slippage_bps) / 10000)
```

For strict receive:

```text
source_max =
  ceil(quoted_source_stroops * (10000 + slippage_bps) / 10000)
```

The derived limit must remain positive and representable by Stellar.

The exact derived `destination_min` or `source_max` is part of the transaction truth. A UI percentage is not the safety boundary.

A quote at zero slippage is valid but more likely to fail after liquidity changes.

## Price and price impact

The stable quote truth is the pair of source/destination amounts plus the selected path.

A product may derive an effective display rate from those amounts.

"Price impact", "spread", or "savings" are **not** Normative quote fields because they require an independent reference price. If a product shows one, it must define the comparison source and must not use that display metric instead of `destination_min` / `source_max`.

## Preparation and review binding

The lifecycle is:

```text
quote -> select quote -> prepare -> review -> confirm -> authorize -> sign -> submit
```

Preparation must bind:

- the selected quote;
- the exact path;
- the fixed amount;
- the quoted opposite amount;
- slippage basis points;
- derived `destination_min` or `source_max`;
- quote expiry;
- transaction TimeBounds;
- fee and network;
- memo when present.

Preparation may re-check current account/provider state. It must not silently replace the selected path or derive a new quote under an already-approved review.

Any change to path, quote amount, slippage limit, destination, memo, fee policy or network requires a new prepared review.

## Account and trustline safety

Preparation must check the applicable current state before presenting the transaction as executable:

- source asset availability after selling liabilities;
- native balance for fee/reserve requirements;
- source trustline authorization for an ordinary issued source asset;
- destination account existence;
- destination trustline/full authorization/capacity for an ordinary issued destination asset;
- issuer special cases required by current Stellar semantics.

Intermediate path assets do not require the source or destination account to hold trustlines for every hop.

Missing destination trustlines are not added implicitly by this Capability. If a product deliberately combines `ChangeTrust` with a same-account Swap, the additional operation, limit, reserve effect and fee must be explicit in the prepared review.

SEP-29 memo-required safety applies to cross-account Path Payments just as it does to ordinary Payment. A same-account Swap normally has no memo requirement.

The network remains authoritative for path liquidity and operation result codes. In particular, a path can still fail because liquidity changed, an issuer/path became invalid, or execution would cross the source account's own offer.

## Swap specialization

A Swap is:

```text
destination_account = source_account
source_asset != destination_asset
```

The same quote, slippage, expiry, review and signing rules apply.

A Swap is intended for immediate bounded conversion. Persistent maker intent remains the SDEX Capability's `ManageBuyOffer` / `ManageSellOffer` domain.

## Submission safety

Path Payment uses the common Transaction and Signing Coordination contracts.

A quote expiring after submission does not prove submission failure. If submission outcome is uncertain, follow normal pending/reconciliation rules. Do not build and submit a fresh quote until the earlier transaction is known not to have landed.

No Flow or provider adapter may rebuild a different Path Payment after the user reviewed and authorized the prepared transaction.

## Errors

Stable semantic categories include:

- `invalid_request`;
- `slippage_out_of_range`;
- `quote_unavailable`;
- `quote_expired`;
- `route_unavailable`;
- `insufficient_source`;
- `destination_missing`;
- `destination_no_trust`;
- `not_authorized`;
- `line_full`;
- `issuer_missing`;
- `cross_self`;
- `slippage_exceeded`;
- authorization/signing/submission errors from the common Transaction/Signing contracts.

Protocol `UNDER_DESTMIN` and `OVER_SENDMAX` map to `slippage_exceeded`.

## Platform boundary

This is a semantic contract, not a shared application implementation.

Mobile/Web may use their platform Stellar SDK and provider client. CLI/TUI/Desktop may use the Rust reference implementation when one is available. Native SDK/FFI continues to own shared cryptographic/security behavior and is not expanded merely to centralize Path Payment networking.

Conforming implementations share the request/quote/slippage/review/safety meaning above.

## Conformance

Cross-language cases are in [`../../spec/test-vectors/path-payment-v1.json`](../../spec/test-vectors/path-payment-v1.json).

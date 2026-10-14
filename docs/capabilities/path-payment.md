# Path Payment / Swap Capability

Maturity: **Normative**

## Purpose

This capability defines atomic Classic-asset conversion with Stellar Path Payment operations.

A Path Payment can convert one asset while paying another Classic account. A **Swap** is the same operation with destination = source. Swap is not a separate ledger primitive and is not an offer workflow.

Scope: XLM and Classic issued assets (`CODE:GISSUER`). Soroban token swaps are outside this contract.

## Modes

`strict_send` fixes the source amount:

```text
fixed: source amount
quote: destination amount
limit: destination_min
operation: PathPaymentStrictSend
```

`strict_receive` fixes the destination amount:

```text
fixed: destination amount
quote: source amount
limit: source_max
operation: PathPaymentStrictReceive
```

For Swap, strict send means **sell exact** and strict receive means **buy exact**.

## Request

A request contains:

- network;
- source and destination Classic `G...` accounts;
- source and destination asset;
- mode;
- fixed-side amount;
- optional transaction memo.

Source and destination assets must differ. Amounts are positive, exact Stellar seven-decimal values; binary floating point must not determine transaction amounts.

The destination must already exist. Path Payment does not inherit Payment Capability's `CreateAccount` behavior. Muxed accounts require a later explicit contract extension.

## Quote

A quote records:

- request identity: network, accounts, assets and mode;
- quoted source amount;
- quoted destination amount;
- ordered intermediate asset path;
- `quoted_at` and `expires_at`;
- observed ledger sequence when the provider exposes one.

`path` contains intermediate assets only; endpoints are omitted. At most five intermediate assets are valid.

The signed operation fixes the asset path, not individual offers or liquidity pools used inside each hop. Do not present a quote as reserved liquidity.

For candidates from the same request, default ordering is:

1. strict send: larger destination amount;
2. strict receive: smaller source amount;
3. fewer intermediate assets;
4. canonical lexical path identity.

A selected candidate must be used exactly during preparation.

## Expiry

A live quote expires within 30 seconds:

```text
expires_at <= quoted_at + 30 seconds
```

Implementations may shorten this window but must not extend it without a new quote.

At or after `expires_at`, preparation returns `quote_expired`. Re-quoting may change amounts or path and requires a new review.

Quote expiry gates **preparation**, not the lifetime of an already prepared transaction.

If preparation completed before expiry, the resulting transaction contains absolute `destination_min` / `source_max` protection and follows the common Transaction Capability TimeBounds policy. This keeps Path Payment compatible with hardware and multisig authorization that may take longer than a quote window.

Products may require a fresher quote before confirmation as UX policy, but must rebuild the review if they do.

Observed ledger sequence is provenance, not a replacement for the expiry rule.

## Slippage

`slippage_bps` is explicit:

```text
0 <= slippage_bps <= 1000
100 bps = 1%
```

The Capability has no hidden default. Product Flows may choose a lower default or maximum, but 1000 bps is Fresnica's cross-product safety ceiling, not a Stellar protocol limit. Raising that ceiling is a contract change, not a UI bypass. The selected value must enter preparation and review.

Limits are derived with integer stroop arithmetic.

Strict send:

```text
destination_min =
  floor(quoted_destination_stroops * (10000 - slippage_bps) / 10000)
```

Strict receive:

```text
source_max =
  ceil(quoted_source_stroops * (10000 + slippage_bps) / 10000)
```

The result must be positive and representable by Stellar. The exact `destination_min` or `source_max` is transaction truth.

A displayed rate may be derived from quoted amounts. Price impact/spread is not Normative because it requires an independent reference price; if shown, it must not replace the ledger slippage limit.

## Prepare and review

Lifecycle:

```text
quote -> select -> prepare -> review -> confirm -> authorize -> sign -> submit
```

Preparation binds the exact:

- selected path and quote amounts;
- fixed amount;
- slippage basis points and derived limit;
- quote expiry and the separately chosen Transaction TimeBounds;
- source/destination accounts and assets;
- fee, network and memo.

Preparation may refresh account/provider state. It must not silently replace the path or quote.

Changing path, quote amount, slippage, destination, memo, fee policy or network requires a new prepared review.

## Safety checks

Before presenting the transaction as executable, check:

- source availability after selling liabilities;
- native fee/reserve availability;
- source trustline authorization for an ordinary issued source asset;
- destination existence;
- destination trustline authorization and capacity for an ordinary issued destination asset;
- applicable issuer special cases.

Intermediate assets do not require source/destination trustlines.

Missing destination trustlines are not added implicitly. If a same-account Swap deliberately includes `ChangeTrust`, that extra operation, limit, reserve effect and fee must be visible in review.

SEP-29 memo-required safety applies to cross-account Path Payments.

The network remains authoritative for liquidity and result codes. A transaction can still fail because liquidity changed, an issuer/path became invalid, or execution would cross the source account's own offer.

## Swap

Swap is the specialization:

```text
destination_account = source_account
source_asset != destination_asset
```

Use the same quote, expiry, slippage, review and signing rules.

Swap is immediate bounded conversion. Persistent maker orders remain part of the SDEX Capability.

## Submission safety

Quote expiry after submission does not prove failure. If submission is uncertain, use normal Transaction reconciliation and do not send a newly quoted replacement until the earlier transaction is known not to have landed.

No layer may rebuild a different Path Payment after review/authorization.

## Errors

Stable categories include:

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
- common authorization/signing/submission errors.

Protocol `UNDER_DESTMIN` and `OVER_SENDMAX` map to `slippage_exceeded`.

## Platform boundary

This contract standardizes semantics, not source code.

Mobile/Web implement the capability with their platform Stellar/provider stack. CLI/TUI/Desktop may use a Rust implementation. Native SDK/FFI remains the shared cryptographic/security boundary; Path Payment networking does not move into it.

## Conformance

Vectors: [`../../spec/test-vectors/path-payment-v1.json`](../../spec/test-vectors/path-payment-v1.json).

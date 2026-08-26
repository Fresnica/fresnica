# SDEX Capability

Maturity: **Normative**

## Purpose

The SDEX Capability defines Fresnica's pair-relative Stellar DEX semantics for offer writes and market/account reads.

Presentation is not standardized. A terminal order book, Mobile trading screen and Desktop market workspace may all conform to the same contract.

## Pair identity

A market pair is always user-facing:

```text
BASE / COUNTER
```

Each asset is full identity:

```text
XLM
or
CODE:GISSUER
```

`BASE` and `COUNTER` must be different.

## User intent

For Fresnica SDEX semantics:

- `amount` means **BASE units**;
- `price` means **COUNTER units per one BASE unit**.

SELL intent maps to:

```text
ManageSellOffer(
  selling = BASE,
  buying = COUNTER,
  amount = BASE amount,
  price = COUNTER / BASE
)
```

BUY intent maps to:

```text
ManageBuyOffer(
  selling = COUNTER,
  buying = BASE,
  buy_amount = BASE amount,
  price = COUNTER / BASE
)
```

The BUY/SELL distinction is semantic and must not be erased by casually inverting a decimal price.

## Numeric rules

- amounts and user prices use exact decimal semantics, not binary floating point;
- the effective Stellar offer price is the encoded positive signed-int32 rational `n/d`;
- exact ledger/order-book price ratios should be retained as integer `n/d` when available;
- a positive nonzero amount/price must not be represented to the user as semantic zero merely because the chosen display precision is too coarse.

### Decimal price rationalization

A decimal price request must be rationalized **before** price-dependent preflight, review and XDR construction. The canonical target is the best positive rational approximation representable by Stellar's signed-int32 `Price { n, d }` bounds, using continued-fraction convergence and a bounded semi-convergent when the next full convergent would exceed the limit but a valid closer fraction still exists.

The resulting `n/d` is the transaction truth. A conforming implementation must use that same effective ratio for:

- BUY-side required-selling/preflight calculations;
- review/effective price and price-dependent totals;
- the final `ManageBuyOffer` / `ManageSellOffer` XDR.

The original decimal may also be retained as user intent. If rationalization changes the value, review must expose enough information to make the effective encoded price clear; it must not display only the requested decimal while signing a materially different `n/d`.

The current Rust reference matches exact/common cases such as `0.325 -> 13/40` but still has an implementation gap at signed-int32 approximation boundaries where semi-convergent recovery is possible. This gap is tracked in [`../tasks.md`](../tasks.md) and does not weaken the contract.

## Reference presentation convention (non-normative)

The current terminal UI renders a positive price below seven-decimal display precision as `<0.0000001` rather than `0.0000000`. Other platforms may use scientific notation, additional precision or another clear representation. The shared requirement is to avoid false-zero meaning, not to copy this string.

## Offer create/update/cancel

### Create

Preparation must preserve BUY/SELL intent, full pair identity, amount/price meaning, reserve/fee capacity and required receiving trustline semantics.

If an implementation offers an explicit "add missing trustline" option, that additional operation must appear in the prepared review and fee/reserve preflight. When that trustline is created implicitly without a user-supplied limit, it must use the canonical Fresnica Trustline limit `708269837873.6765`. Review must expose both the receiving asset and the effective trustline limit because the additional `ChangeTrust` is part of the transaction being authorized.

### Update

Updating an existing offer must preserve its actual market pair and BUY/SELL direction. Implementations must derive direction from canonical on-chain selling/buying assets instead of guessing from a displayed reciprocal price.

### Cancel

Cancellation must target the exact owned offer identity and preserve the operation family/direction required by Stellar for that offer.

A product must not cancel an offer owned by another account.

## Open offers

Canonical ledger offers contain selling/buying assets, remaining selling amount and exact price ratio. They are chain state, not guaranteed storage of original user intent.

When projecting an offer into a selected pair:

- canonical `selling=BASE,buying=COUNTER` projects as SELL;
- canonical `selling=COUNTER,buying=BASE` projects as BUY using the reciprocal exact ratio;
- after partial BUY fills, displayed remaining BASE amount is a projection from canonical remaining selling amount, not a recoverable original order amount.

## Order book

For a `BASE / COUNTER` book, the normalized Capability semantics are:

- BID corresponds to BUY BASE;
- ASK corresponds to SELL BASE;
- level amount is BASE units on both sides;
- level price is COUNTER per BASE;
- exact `n/d` should be retained where available.

Provider adapters own any transport-specific conversion needed to produce that normalized meaning. For example, the current Horizon adapter receives bid amount in the counter-side canonical representation and converts it to BASE using the exact ratio; this Horizon field shape is reference transport behavior, not part of the cross-platform contract.

## Trades and fills

Pair trades expose BASE amount, COUNTER amount, price and BASE-side BUY/SELL meaning.

Account trade compression may merge consecutive trade records only when they can be proven to represent the same user offer segment with the same:

- pair;
- side;
- exact price fraction;
- identified user offer ID.

Missing offer IDs do not merge. Equal-looking records separated by another segment do not merge across the gap.

## Candles

Candle/aggregation transports may vary. Where exposed, the semantic result includes pair identity, resolution, timestamp, OHLC, BASE volume and trade count. Provider-specific pagination/offset mechanics are not UI semantics.

## Review and signing

Offer writes use the common Transaction and Signing Coordination contracts. A prepared review must expose the actual operation family (`ManageBuyOffer`/`ManageSellOffer`), pair, side, amount, **effective encoded price**, price-dependent total, fee, network and any extra trustline effect. When a decimal request was rationalized, the review contract must be able to distinguish requested price from effective `n/d`; when a receiving trustline is added, the review must include its asset and effective limit.

## Conformance

Normative pair/offer/fill semantics are captured in [`../../spec/test-vectors/sdex-v1.json`](../../spec/test-vectors/sdex-v1.json).

The current Rust reference implementation is `clients/rust-client::dex`.

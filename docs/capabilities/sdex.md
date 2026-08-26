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
- exact Stellar offer/order-book price ratios should be retained as integer `n/d` when available;
- user-facing seven-decimal numeric projection follows the Fresnica conformance vectors where such projection is used;
- a positive nonzero amount/price must not be represented to the user as semantic zero merely because the chosen display precision is too coarse.


## Reference presentation convention (non-normative)

The current terminal UI renders a positive price below seven-decimal display precision as `<0.0000001` rather than `0.0000000`. Other platforms may use scientific notation, additional precision or another clear representation. The shared requirement is to avoid false-zero meaning, not to copy this string.

## Offer create/update/cancel

### Create

Preparation must preserve BUY/SELL intent, full pair identity, amount/price meaning, reserve/fee capacity and required receiving trustline semantics.

If an implementation offers an explicit "add missing trustline" option, that additional operation must appear in the prepared review and fee/reserve preflight.

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

For a `BASE / COUNTER` book:

- BID corresponds to BUY BASE;
- ASK corresponds to SELL BASE;
- both displayed level amounts are BASE units;
- Horizon bid amount is counter-side canonical amount and must be normalized to BASE using the exact ratio;
- ask amount is already BASE amount for the requested pair.

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

Offer writes use the common Transaction and Signing Coordination contracts. A prepared review must expose the actual operation family (`ManageBuyOffer`/`ManageSellOffer`), pair, side, amount, price, total, fee, network and any extra trustline effect.

## Conformance

Normative pair/offer/fill semantics are captured in [`../../spec/test-vectors/sdex-v1.json`](../../spec/test-vectors/sdex-v1.json).

The current Rust reference implementation is `clients/rust-client::dex`.

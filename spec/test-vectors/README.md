# Fresnica cross-language test vectors

These files define language-neutral behavior that future Fresnica cores and clients must agree on. They are semantic fixtures, not serialized Python objects and not a public wire protocol.

## Versioning

`sdex-v1.json` is the first SDEX behavior set. Existing vector meaning must not be changed silently. If a later implementation intentionally changes a normative behavior, add a new version and document the compatibility decision.

## Numeric rules

- Asset amounts and human prices are JSON strings containing base-10 decimals. Implementations must not pass them through binary floating point.
- Exact Stellar prices are represented as integer fractions: `{ "n": ..., "d": ... }`.
- User-facing projected values follow Stellar's seven-decimal precision and Fresnica's current half-up projection rule.
- Tests should compare decimal values numerically; insignificant trailing zeroes are not semantic.

## Market and offer semantics

A `pair` is always user-facing `BASE / COUNTER`.

- `amount` always means BASE units.
- `price` always means COUNTER units per one BASE unit.
- SELL encodes `ManageSellOffer(selling=BASE, buying=COUNTER, amount=BASE amount, price=COUNTER/BASE)`.
- BUY encodes `ManageBuyOffer(selling=COUNTER, buying=BASE, buyAmount=BASE amount, price=COUNTER/BASE)`.

A ledger `OpenOffer` is canonical chain state, not stored user intent. When canonical selling/buying are reversed relative to the selected pair, Fresnica projects it as a BUY by using the exact reciprocal price fraction. After partial fills, the displayed remaining BUY amount is a projection from canonical remaining selling amount; implementations must not treat it as a recoverable original order intent.

## Account trade aggregation

Account trades are compressed only when consecutive records can be proven to be fills from the same user offer at the same exact price fraction.

The normative merge identity is:

- market pair
- side
- exact price fraction
- identified user offer ID

Missing user offer IDs never merge. Equal records separated by another segment never merge across that gap. This deliberately avoids grouping unrelated AMM/path-payment activity merely because pair, side, and decimal price happen to match.

## What is deliberately not normative

Implementation-private values such as Python's `segment_key`, cache keys, database schemas, UI strings, class names, and storage layout are excluded. Rust, Python, and mobile implementations may organize those differently as long as they produce the same semantic results in the vectors.

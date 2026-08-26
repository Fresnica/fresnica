# Balance / Availability Capability

Maturity: **Normative**

## Purpose

The Balance / Availability Capability defines the wallet meaning of balances, liabilities, reserves and spendable/committable amounts used by transaction-producing Flows.

It is not a requirement to expose Horizon's raw balance JSON.

## Asset identity

Full asset identity is authoritative:

```text
native XLM
or
CODE:GISSUER
```

Code-only issued-asset identity is insufficient.

## Numeric rule

Classic Stellar asset amounts use seven-decimal fixed precision. Semantic calculations must use exact decimal/integer-stroop arithmetic rather than binary floating point.

## Native availability

When preparing a native spend, availability must account for at least:

- current XLM balance;
- XLM selling liabilities;
- current minimum balance/reserve requirement;
- transaction fee for the prepared transaction.

For the current Classic account model, minimum-balance reserve units are:

```text
reserve_units = max(0, 2 + subentry_count + num_sponsoring - num_sponsored)
minimum_balance = reserve_units * current_base_reserve
```

Products must use current network/ledger reserve values rather than hard-coded historical constants. Sponsorship fields are part of the reserve calculation; `subentry_count` alone is not a complete minimum-balance formula.

Transaction preflight must evaluate the prepared operation set's expected reserve footprint rather than only the account's current `subentry_count`. For example, adding a trustline or creating a new offer may require temporary/new reserve capacity, while removing/cancelling an entry may release it. The operation-specific Capability defines the exact ledger effect.

## Issued-asset availability

For an issued asset held through a trustline, transferable availability must account for at least:

- current asset balance;
- selling liabilities;
- required XLM fee availability;
- applicable trustline/account constraints.

The issuer of an issued asset is a protocol special case: it does not hold its own trustline, so its ability to issue/send that asset must not be computed as `missing trustline -> zero balance`.

## Receiving capacity

Preflight that can increase balance/buying liabilities must also reason about receiving capacity. For an ordinary issued-asset trustline:

```text
receiving_capacity = limit - balance - buying_liabilities
```

This capacity is meaningful only together with the trustline's current authorization state. A fully authorized trustline can receive/create new liabilities subject to capacity; `AUTHORIZED_TO_MAINTAIN_LIABILITIES` can maintain/reduce existing liabilities but does not authorize ordinary new receipt/offer creation.

Native XLM receiving capacity is also finite because ledger amounts are signed 64-bit stroops:

```text
native_receiving_capacity = INT64_MAX - balance - buying_liabilities
```

Do not model native receipt as mathematically unlimited. Issuer-side semantics for its own issued asset are again special and do not require an issuer self-trustline.

## Relationship to Flows

A Portfolio Flow may display additional information, formatting or fiat estimates. Those presentation choices are not normative.

Payment, Trustline and SDEX Capabilities consume Balance / Availability semantics for preflight decisions; they must not each invent incompatible reserve/liability formulas.

## Errors

Flows should be able to distinguish insufficient asset balance, insufficient XLM fee/reserve capacity and malformed/unavailable network state without parsing transport-specific strings.

## Reference extension: liquidity-pool portfolio projection (non-normative)

RefPython already treats `liquidity_pool_shares` as a distinct portfolio position rather than pretending pool shares are an ordinary `CODE:GISSUER` balance:

- [`reference/python/fresnica/balance_service.py`](../../reference/python/fresnica/balance_service.py)
- [`reference/python/tests/test_pool_cache.py`](../../reference/python/tests/test_pool_cache.py)

The current reference preserves the liquidity-pool identity and share balance, loads the pool's total shares/reserves, and derives the user's underlying reserve position proportionally:

```text
share_ratio = owned_pool_shares / total_pool_shares
underlying_reserve_amount = pool_reserve_amount * share_ratio
```

Pool details are cached by network + pool identity. If a live pool-detail lookup fails after a prior successful lookup, the reference can still build a position from cached pool state rather than dropping the position entirely.

These are promising portfolio semantics but are not yet part of the Normative Balance contract. Mobile/Web/Desktop evidence should determine whether liquidity-pool position projection becomes a normative extension of Balance or a separate Capability. Liquidity-pool shares must not silently become normal payment/trustline/SDEX asset identity merely because they appear in an account balance response.

## Reference evidence

The reserve/availability semantics above are exercised independently by the Python and Rust references:

- [`reference/python/fresnica/availability.py`](../../reference/python/fresnica/availability.py)
- [`reference/python/tests/test_availability.py`](../../reference/python/tests/test_availability.py)
- [`clients/rust-client/src/transaction.rs`](../../clients/rust-client/src/transaction.rs)
- [`clients/rust-client/src/payment.rs`](../../clients/rust-client/src/payment.rs)

The current Rust implementation also applies the same availability primitives to Payment, Trustline and SDEX preflight. A future normalized cross-platform balance DTO may be standardized separately without changing these invariants.

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

## Issued-asset availability

For an issued asset, transferable availability must account for at least:

- current asset balance;
- selling liabilities;
- required XLM fee availability;
- applicable trustline/account constraints.

## Relationship to Flows

A Portfolio Flow may display additional information, formatting or fiat estimates. Those presentation choices are not normative.

Payment, Trustline and SDEX Capabilities consume Balance / Availability semantics for preflight decisions; they must not each invent incompatible reserve/liability formulas.

## Errors

Flows should be able to distinguish insufficient asset balance, insufficient XLM fee/reserve capacity and malformed/unavailable network state without parsing transport-specific strings.

## Reference evidence

The reserve/availability semantics above are exercised independently by the Python and Rust references:

- [`reference/python/fresnica/availability.py`](../../reference/python/fresnica/availability.py)
- [`reference/python/tests/test_availability.py`](../../reference/python/tests/test_availability.py)
- [`clients/rust-client/src/transaction.rs`](../../clients/rust-client/src/transaction.rs)
- [`clients/rust-client/src/payment.rs`](../../clients/rust-client/src/payment.rs)

The current Rust implementation also applies the same availability primitives to Payment, Trustline and SDEX preflight. A future normalized cross-platform balance DTO may be standardized separately without changing these invariants.

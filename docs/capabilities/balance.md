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

The current Classic minimum-balance interpretation is based on Stellar reserve units including base account units, subentries and sponsorship adjustments. Products must use current network/ledger values rather than hard-coded historical reserve constants.

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

## Reference status

The current Rust implementation performs exact stroop parsing, reserve/liability-aware payment preflight and SDEX/trustline capacity checks. A future normalized cross-platform balance DTO may be standardized separately without changing these invariants.

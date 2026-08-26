# History / Activity Capability

Maturity: **Defined**

## Purpose

History / Activity is the shared capability name for account-scoped wallet activity and transaction/operation history.

It is currently **Defined**, not Normative, because Fresnica does not yet have one stable cross-platform normalized activity DTO. The Rust engineering client still exposes Horizon-shaped operation records, while the Python reference has a richer local cache/presentation model.

## Agreed boundary

All implementations must preserve:

- account + network scope;
- stable chain identity such as paging token/transaction/operation identifiers where available;
- chronological/paging meaning;
- no cache leakage between networks or accounts;
- separation between raw chain truth and product presentation metadata.

A product may enrich activity with contacts, labels or local metadata, but those enrichments must not overwrite the underlying chain identity.

## Storage/cache policy

Cache layout, retention count, paging strategy, local database schema and refresh policy are implementation-specific until promoted into a stronger contract.

The current terminal/Python behavior is reference material, not a universal requirement.

## Promotion criteria

Promote History / Activity to Normative when Mobile/terminal/Web experience yields a stable cross-platform activity model and paging/result semantics worth sharing.

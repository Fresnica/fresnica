# History / Activity Capability

Maturity: **Defined**

## Purpose

History / Activity is the shared capability name for account-scoped wallet activity and transaction/operation history.

It is currently **Defined**, not Normative, because Fresnica does not yet have one stable cross-platform normalized activity DTO. The Rust engineering client still exposes Horizon-shaped operation records, while the Python reference has a richer cache, grouping and presentation model.

`Defined` does not mean that existing behavior is discarded. Mature implementation experience is recorded below as **Reference Semantics** so that Mobile, Web, Desktop and later Rust implementations can reuse it, challenge it with concrete evidence, and eventually promote stable parts into the Normative contract.

## Agreed boundary

All implementations must preserve:

- account + network scope;
- stable chain identity such as paging token/transaction/operation identifiers where available;
- chronological/paging meaning;
- no cache leakage between networks or accounts;
- separation between raw chain truth and product presentation metadata.

A product may enrich activity with contacts, labels or local metadata, but those enrichments must not overwrite the underlying chain identity.

## Reference Semantics: Python implementation

The Python reference is the strongest current product implementation of this capability:

- [`reference/python/fresnica/history_service.py`](../../reference/python/fresnica/history_service.py)
- [`reference/python/tests/test_history_sync.py`](../../reference/python/tests/test_history_sync.py)
- [`reference/python/tests/test_history_ux.py`](../../reference/python/tests/test_history_ux.py)
- [terminal history-cache design](../platforms/terminal/history-cache.md)

Its behavior should be treated as **implementation evidence and candidate shared semantics**, not as an automatically Normative API.

### 1. Raw chain history and product activity are separate layers

The reference stores raw account operations as the durable synchronization unit, then derives higher-level `ActivityView` objects for presentation.

Conceptually:

```text
chain operations
      |
      v
raw account-scoped cache
      |
      +--> transaction grouping
      +--> summaries / counterparties
      +--> spam heuristics
      +--> contacts / issuer metadata
      |
      v
product activity views
```

This separation is a strong candidate for cross-platform adoption because it prevents presentation changes from destroying chain identity or synchronization state.

### 2. Synchronization is cursor-based and incremental

The reference uses a bounded recent-history cache by default:

- an empty cache starts at the current chain head and walks backwards;
- once local history exists, refresh starts from the newest cached cursor and walks forward to the current head;
- refresh has no arbitrary fixed page-count catch-up limit;
- account and network are always part of the cache scope.

An optional full-history mode keeps locally discovered older records and backfills as far as the connected upstream provider still exposes them. It never claims to reconstruct records already pruned upstream.

The exact provider cursor and retention policy are not yet cross-platform requirements, but the **incremental synchronization meaning** and the distinction between local retention and upstream availability are useful candidate semantics.

### 3. Transaction-level activity is derived from operations

The Python reference groups operations sharing a `transaction_hash` into one activity while preserving the child operations. Operations without a transaction hash retain their own stable operation identity.

A derived activity can therefore expose concepts such as:

```text
transaction identity
created time
operation count
ordered child operations
summary / presentation metadata
```

without replacing the underlying operations.

This is a stronger candidate for a future normalized Activity DTO than exposing provider-shaped operation JSON directly.

### 4. Enrichment is derived, not chain truth

The reference derives additional UX information from cached operations, including:

- human-readable operation/transaction summaries;
- counterparties;
- contact names;
- issuer-domain metadata;
- contract asset-balance-change summaries;
- suspicious unsolicited claimable-balance heuristics.

Tests deliberately protect cases where a claimable-balance operation is part of a legitimate mixed transaction so that spam filtering does not hide meaningful sibling operations.

These behaviors are valuable implementation references, but specific summary strings, spam thresholds and display policy remain product-level choices until cross-platform experience shows stable common semantics.

## Candidate semantics for promotion

The following Python-reference behavior is especially worth testing in Mobile/Web/Desktop before promoting History to Normative:

1. Preserve a raw chain-record layer independently from derived activity presentation.
2. Derive transaction-level activities by stable transaction identity while retaining child operations.
3. Keep synchronization/account identity scoped by network + account.
4. Refresh incrementally from a stable chain cursor rather than rebuilding history on every view.
5. Treat local cache retention and upstream historical availability as separate concepts.
6. Apply contacts, labels, spam classification and other enrichment without mutating raw chain truth.
7. Make filtering/presentation operate on derived activity while retaining enough raw information to explain or re-render the result.

If independent platform implementations converge on these semantics, they should be promoted into the Normative History contract and backed by shared conformance fixtures.

## Implementation-specific choices today

The following Python/terminal choices are **not** currently part of the common contract:

- Horizon as the required upstream provider;
- raw Horizon operation JSON as the universal stored DTO;
- paging tokens as the only possible cursor representation;
- the default `2,000` operation retention count;
- a boolean `keep full history locally` preference;
- Python datastore schema or serialization;
- exact summary text;
- the current suspicious-claimable thresholds and heuristics;
- terminal keys or pagination UI.

Another platform may use Stellar RPC, a normalized local database, a different retention strategy or different UX while still learning from the reference model.

## Storage/cache policy

Cache layout, retention count, provider-specific paging mechanics, local database schema and refresh timing remain implementation-specific until promoted into a stronger contract.

A platform implementation should nevertheless document which Reference Semantics it adopts or intentionally rejects and why. A separate product such as `fresnica-mobile` may contribute that evidence through a documentation PR linking to its own implementation/tests. Concrete divergence is useful input for evolving this Capability.

## Promotion criteria

Promote History / Activity to Normative when independent implementations provide enough evidence to freeze a common model for:

- raw history identity;
- normalized activity/result semantics;
- grouping rules;
- paging/synchronization meaning;
- cache/upstream-history distinction;
- enrichment boundaries.

Promotion should preserve useful RefPython behavior where it survives cross-platform review rather than designing a replacement from scratch.

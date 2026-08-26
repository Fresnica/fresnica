# History Cache Model

Fresnica treats account History as a local wallet cache, not as a complete blockchain indexer.

## Default mode

The default retention limit is **2,000 Horizon operations per account and network**.

When no local history exists:

```text
Horizon head
    |
    v  order=desc
newest -> older -> ... -> 2,000 operations
```

Fresnica starts at the current Horizon head and walks backwards until it has cached 2,000 operations or Horizon has no more records.

When local history already exists:

```text
local newest cursor
    |
    v  order=asc
older -> newer -> ... -> current Horizon head
```

Refresh starts from the newest locally cached paging token and walks only forward. As new pages are stored, the oldest cached rows are discarded so the cache remains bounded at 2,000 operations. There is no fixed page-count catch-up limit.

This means a wallet that has been offline for a long time still uses one direction for incremental synchronization: old to new. High-frequency accounts may require more requests, but the local storage bound remains predictable.

## Full local history

`Keep full history locally` is a boolean opt-in. It is deliberately not a numeric cache-size setting.

When enabled:

- Fresnica stops trimming old operations.
- Refresh still catches up from the newest local cursor to the current head.
- Fresnica also walks backwards from the oldest local cursor until the connected Horizon instance has no older records available.
- Records already pruned by Horizon cannot be reconstructed. Records Fresnica has already cached can remain available locally after upstream pruning.

When disabled again, the next synchronization returns the cache to the newest 2,000 operations.

## UI behavior

History initially displays a smaller local slice for readability. `M Older` reveals more already-cached activity; it does not issue a separate Horizon request. `F Full history` toggles the persisted full-history preference and synchronizes using the selected model.

The status line tells the user whether the cache is `recent 2,000 ops` or `full available history`.

## Invariants

- Cache keys remain scoped by Stellar network and account.
- The retained unit is a Horizon **operation**, not a grouped UI activity.
- Paging tokens are the synchronization boundary.
- Default storage is bounded by operation count, not approximate file size.
- History does not use a `caught_up` product state or a fixed number of incremental pages.
- Transaction grouping, spam presentation, contacts, and other History presentation remain derived from cached raw operations.

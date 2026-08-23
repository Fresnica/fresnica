# Architecture Decision Log

This file records design decisions that materially change Fresnica behavior or boundaries. Git history remains the detailed implementation record.

## 2026-08-23 - History uses retention, not bounded catch-up state

**Decision:** Keep the newest 2,000 Horizon operations by default, with a boolean `Keep full history locally` opt-in.

**Synchronization model:**

- Empty cache: start at Horizon head and walk backwards to the retention target.
- Existing cache: start from the newest local paging token and walk forward to the current head.
- Default mode: trim oldest rows while new rows arrive.
- Full-history mode: do not trim and backfill whatever older history Horizon still exposes.

**Supersedes:** the short-lived PR #39 History model that exposed `SyncResult(caught_up)` and stopped incremental refresh after five 200-operation pages. That model treated local History too much like a bounded indexer catch-up job. The retained product model is simpler: one incremental direction, predictable default storage, and an explicit full-history opt-in.

**Why 2,000:** it is large for ordinary wallet activity but still gives the default cache a clear storage bound. A numeric user-configurable limit was rejected as unnecessary UI complexity.

See [history-cache.md](history-cache.md).

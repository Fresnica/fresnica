# Architecture Decision Log

This file records design decisions that materially change Fresnica behavior or boundaries. Git history remains the detailed implementation record.

## 2026-08-24 - Password is a protection provider, not a wallet model

**Decision:** Separate account identity, signing capability, local secret material, and secret protection. Password protection becomes one `ProtectionProvider` implementation rather than a requirement embedded in `WalletManager` semantics.

The Python reference keeps password protection compatible with the existing Scrypt + AES-256-GCM envelope and adds a generic `SystemProtectionProvider` boundary backed by an injected `SystemKeyStore`. The reference intentionally does not bind this interface to platform biometric APIs.

**System authentication model:** biometrics or OS login authorize access to an OS-protected key; biometric data is never used as encryption key material.

**Signer boundary:** hardware/external signers remain independent of local secret protection. Future Stellar contract-account/passkey wallets should use their own account/signer implementation rather than being forced into classic-account password semantics.

Existing password envelopes remain readable. Explicit migration only adds provider metadata around the existing encrypted envelope and does not re-encrypt secret material.

See [Wallet Protection Model](protection.md).

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

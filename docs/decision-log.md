# Architecture Decision Log

This file records design decisions that materially change Fresnica behavior or boundaries. Git history remains the detailed implementation record.

## 2026-08-24 - One app passcode; system authentication authorizes signers

**Decision:** ordinary local software wallets use one Fresnica app passcode at the product level. Each wallet remains independently protected by Core using its own random salt and nonce, so the same app passcode does not imply one shared wallet AES key.

**Core boundary:** Rust Core is authoritative for secret payloads, KDF/cipher semantics, wallet identity validation, signer construction, and transaction signing. Mobile persists Core-generated encrypted envelopes as opaque data and must not duplicate wallet cryptography.

**Mobile boundary:** Keychain / Keystore, biometric UI, app lock/session state, Realm/database encryption, and platform lifecycle stay in the mobile layer. Xaman platform infrastructure may be reused for these responsibilities.

**System authentication:** Face ID, Touch ID, Android biometrics, Windows Hello, device passcode, and similar facilities are signer-authorization mechanisms. They must not create a second independently encrypted copy of a software wallet. For local software signers, system authentication may authorize access to Core-compatible unlock material for the same canonical wallet envelope. For hardware/external/future signers, it may authorize invocation without any local private key.

**Supersedes part of the earlier protection-provider decision:** `SystemProtectionProvider` / `SystemKeyStore` remains current prototype code, but a mutually exclusive `system` wallet protection kind is no longer the target mobile product model. Password protection remains the canonical local software-wallet envelope; system authentication moves to the signer-authorization/platform boundary.

**Pre-release migration:** Fresnica has not had a public wallet release, so current internal test wallet files do not require compatibility migration code. After public release, every persisted-format change must ship with an explicit versioned, recoverable migration path.

See [Mobile / Rust Core Vault Contract](mobile-core-contract.md) and [Wallet Protection Model](protection.md).

## 2026-08-24 - Account identity is not permanently tied to G addresses

**Decision:** Fresnica distinguishes classic `G...` accounts from contract `C...` accounts at the domain-model boundary.

The Python reference continues to implement classic-account behavior only. It may represent and validate a contract-account identity, but it does not implement Soroban RPC, Stellar Asset Contract behavior, SEP-45, contract authorization, or passkey smart-wallet signing.

**Signer boundary:** the current `Signer.public_key + sign(transaction)` contract remains a classic Ed25519 signer interface. Future contract/passkey authorization must use an appropriate contract signer/auth model rather than pretending a C account has a classic public key.

This keeps current wallet behavior stable while preventing the future Rust Core from encoding `account == G address == Ed25519 public key` as a universal invariant.

See [Signer Architecture](signer.md).

## 2026-08-24 - Password is a protection provider, not a wallet model

**Decision:** Separate account identity, signing capability, local secret material, and secret protection. Password protection becomes one `ProtectionProvider` implementation rather than a requirement embedded in `WalletManager` semantics.

The Python reference keeps password protection compatible with the existing Scrypt + AES-256-GCM envelope and adds a generic `SystemProtectionProvider` boundary backed by an injected `SystemKeyStore`. The reference intentionally does not bind this interface to platform biometric APIs.

**System authentication model:** biometrics or OS login authorize access to an OS-protected key; biometric data is never used as encryption key material.

**Signer boundary:** hardware/external signers remain independent of local secret protection. Future Stellar contract-account/passkey wallets should use their own account/signer implementation rather than being forced into classic-account password semantics.

Existing password envelopes remain readable. Explicit migration only adds provider metadata around the existing encrypted envelope and does not re-encrypt secret material.

**Later refinement:** the accepted Mobile/Core vault contract keeps the separation principle but moves system authentication out of the user-facing wallet `ProtectionProvider` choice and into signer authorization. See the newer decision above.

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

# Architecture Decision Log

This file records design decisions that materially change Fresnica behavior or boundaries. Git history remains the detailed implementation record.

## 2026-08-26 - One device System Auth Domain; Passcode remains higher authority; mnemonic HD derivation stays inside Core

**Decision:** Mobile initializes system authentication once per device/application installation, not once per signer. The domain owns one auth-bound private wrapping key and a public wrapping key. Each protected software signer continues to have its own Core envelope, salt and `WalletUnlockKey`; the domain only wraps those independent unlock keys. No global Vault Master Key is introduced.

**Initialization:** `initializeSystemAuth` proves the domain private key is genuinely protected by the platform policy through one authenticated challenge.

**Later signers:** after the user proves the Fresnica app passcode, Core derives/verifies that signer's `WalletUnlockKey` and native code wraps it with the existing domain public key. `registerSignerSystemAuth` therefore does not trigger another biometric prompt. Routine `signWithSystemAuth` requires private-key unwrap and is the use-time biometric/system-auth boundary.

**Authority:** `Passcode > System Auth`. System auth may authorize routine signing but cannot authorize Reveal/Export, passcode change, or replacement of the recovery root. Face ID/fingerprint success is not proof of the Fresnica passcode.

**Passcode rotation:** Mobile stages Core `reprotect` for every protected software signer, writes nothing if any stage fails, atomically commits the complete new envelope set, then derives/registers the new verified unlock keys into the existing device domain. Post-commit wrapped-key registration does not require another biometric prompt and is retryable.

**HD mnemonic accounts:** the first mnemonic signer normally uses index 0. `derive_mnemonic_signer` authenticates an existing mnemonic-backed protected source and derives an explicit later index inside Core, returning a new protected signer envelope without returning the mnemonic to Mobile/JavaScript. A secret-backed source is rejected. Mobile may group related signers under a Recovery Source for backup/UX purposes; **Account != Signer != Recovery Source**.

**Release boundary:** these changes were introduced in Native SDK v0.2.0 (Native Binding API 2 / SDK API 3 / Core Client API 3 / RN adapter source 0.2.0). The final Mobile handoff baseline is v0.2.1; it keeps the same API constants and adapter source while correcting device-domain failure/cleanup atomicity and aligning the handoff documentation.

See [Mobile System Authentication](mobile-system-auth.md), [Mobile / Rust Core Vault Contract](mobile-core-contract.md), [Wallet Protection Model](protection.md), and [Mobile SDK Usage](mobile-sdk-usage.md).

## 2026-08-24 - OS authorization belongs to clients; Core accepts WalletUnlockKey

**Decision:** Rust Core does not implement, abstract, or store operating-system authentication state. TUI/CLI, desktop, mobile, and future clients own Keychain/Keystore/platform credentials, biometrics, Windows Hello, PAM, session policy, and other OS-specific authorization behavior.

**Software-wallet credential:** the standard Core credential for routine software signing is `WalletUnlockKey`, the exact 32-byte Scrypt output for the canonical password-protected wallet envelope. A client may protect this value with any suitable OS mechanism, but Core only receives the resulting 32-byte key.

**Enrollment:** clients obtain an unlock key through a verified Core path: app passcode + canonical envelope -> Scrypt key -> decrypt -> reconstruct signer -> verify expected public key -> return `WalletUnlockKey`. This prevents enrollment of an unlock key for substituted or mismatched wallet material.

**Signing:** clients submit the canonical encrypted envelope plus `WalletUnlockKey`; Core decrypts the same envelope, re-validates wallet identity, signs, and drops secret-bearing state. No second wallet ciphertext and no independent system wallet key are created.

**Disclosure:** `WalletUnlockKey` cannot authorize Reveal / Export. Explicit secret disclosure continues to require a fresh Fresnica app passcode and the dedicated export path.

**Re-keying:** changing the app passcode or re-encrypting with a new salt changes the unlock key. Clients must invalidate and re-enroll any OS-protected copy of the previous key.

**Supersedes:** the Rust `SystemProtectionProvider` / `SystemKeyStore` prototype and the earlier idea that Core should model a peer `system` protection kind. Those OS concerns now live exclusively in clients.

See [Client / Rust Core Security Contract](client-core-security.md), [Wallet Protection Model](protection.md), and [Signing Material Reveal and Export](secret-export.md).

## 2026-08-24 - Signing-material export is an explicit declassification boundary

**Decision:** normal signing must keep mnemonic/private signing material inside Rust Core. User-requested Reveal / Export is a separate high-risk operation that may return the original recoverable signing material only after fresh Fresnica app-passcode authentication and Core identity verification.

**Authentication:** Face ID, Touch ID, another system-auth success, or an already-unlocked application session is not sufficient by itself to disclose a mnemonic or private key. System authentication authorizes signer use; secret disclosure changes the confidentiality boundary and therefore requires explicit passcode re-entry.

**Material semantics:** mnemonic-backed wallets may reveal the stored mnemonic plus any mnemonic passphrase and derivation metadata required to reconstruct the account. Secret-key-backed wallets may reveal the stored Stellar `S...` secret. A secret key must never be presented as if its original mnemonic can be reconstructed. External/hardware/remote signers cannot export private material that Fresnica never possessed.

**Client handling:** revealed plaintext must not be persisted, logged, sent to analytics, automatically copied to the clipboard, or retained after the export flow. Normal transaction signing must not reuse the export API.

See [Signing Material Reveal and Export](secret-export.md) and [Client / Rust Core Security Contract](client-core-security.md).

## 2026-08-24 - One app passcode; system authentication authorizes signers

**Decision:** ordinary local software wallets use one Fresnica app passcode at the product level. Each wallet remains independently protected by Core using its own random salt and nonce, so the same app passcode does not imply one shared wallet AES key.

**Core boundary:** Rust Core is authoritative for secret payloads, KDF/cipher semantics, wallet identity validation, signer construction, and transaction signing. Clients persist Core-generated encrypted envelopes as opaque data and must not duplicate wallet cryptography.

**Client boundary:** secure storage, biometric UI, app lock/session state, database encryption, and platform lifecycle stay in the client layer.

**System authentication:** OS authentication is a signer-authorization mechanism. For software wallets it may authorize release of a client-protected Core `WalletUnlockKey`; for hardware/external/future signers it may authorize invocation without any local private key.

**Later refinement:** the concrete cross-client contract is now fixed by the newer decision above: Core accepts `WalletUnlockKey` and no longer contains a `SystemProtectionProvider` product path.

**Pre-release migration:** Fresnica has not had a public wallet release, so current internal test wallet files do not require compatibility migration code. After public release, every persisted-format change must ship with an explicit versioned, recoverable migration path.

See [Client / Rust Core Security Contract](client-core-security.md) and [Wallet Protection Model](protection.md).

## 2026-08-24 - Account identity is not permanently tied to G addresses

**Decision:** Fresnica distinguishes classic `G...` accounts from contract `C...` accounts at the domain-model boundary.

The Python reference continues to implement classic-account behavior only. It may represent and validate a contract-account identity, but it does not implement Soroban RPC, Stellar Asset Contract behavior, SEP-45, contract authorization, or passkey smart-wallet signing.

**Signer boundary:** the current `Signer.public_key + sign(transaction)` contract remains a classic Ed25519 signer interface. Future contract/passkey authorization must use an appropriate contract signer/auth model rather than pretending a C account has a classic public key.

This keeps current wallet behavior stable while preventing the future Rust Core from encoding `account == G address == Ed25519 public key` as a universal invariant.

See [Signer Architecture](signer.md).

## 2026-08-24 - Password is a protection provider, not a wallet model

**Decision:** Separate account identity, signing capability, local secret material, and secret protection. Password protection is a Core protection mechanism rather than a requirement embedded in `WalletManager` semantics.

The password envelope remains Scrypt + AES-256-GCM. Hardware/external signers remain independent of local secret protection, and future Stellar contract-account/passkey wallets should use their own account/signer implementation rather than being forced into classic-account password semantics.

**Historical note:** an early Python/Rust prototype modeled system protection as another `ProtectionProvider`. The later Client/Core decisions supersede that product direction: OS authorization is client-owned and the canonical software-wallet envelope remains password-protected.

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

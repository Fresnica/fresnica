# TUI / CLI System Authorization

Status: **Rust client + Terminal one-shot signing path validated; production OS backends are platform-specific and not yet shipped**.

This document applies the [Client / Rust Core Security Contract](../../core/client-security.md) to the current Fresnica TUI/CLI.

## Boundary

The TUI never gives an OS API to Rust Core and never reimplements wallet cryptography.

```text
TUI / CLI
  |
  +-- OS-specific SystemUnlockBackend
  |       |
  |       +-- enroll WalletUnlockKey
  |       +-- authenticate + release WalletUnlockKey
  |       +-- delete enrollment
  |
  +-- WalletManager / Rust Core boundary
          |
          +-- derive_verified_unlock_key(passcode)
          +-- unlock/sign(WalletUnlockKey)
          +-- reveal/export(fresh passcode)
```

The Rust reference client now owns the platform-neutral `SystemAuthSlot` and System Auth signing source. Terminal injects a client-owned backend; the released protected envelope is still signed only through Fresnica SDK/Core. No OS API or KDF/decryption logic moved into Terminal or Core.

CLI system authentication is one-shot. Non-interactive CLI invocation does not activate System Auth. Session lifetime remains an application/TUI concern rather than a CLI credential cache.

## TUI behavior

When a backend is available:

1. Wallet Management shows **Enable system unlock** for local software wallets.
2. Enrollment requires a fresh Fresnica app passcode.
3. Fresnica derives and identity-verifies the wallet's canonical 32-byte unlock key.
4. Only that unlock key is passed to the OS backend.
5. Later unlock requests first ask the backend to perform its local authorization and release the key.
6. The released key opens the same canonical password envelope.
7. If no exact enrollment exists or no System Auth backend is available, the client may use the fresh Passphrase path.
8. Once an enrolled System Auth provider is invoked, cancellation, provider failure, stale enrollment, or an invalid released key fails the current attempt; it must not silently downgrade to a Passphrase prompt.
9. If the first signing plan instead reports that selected local software signers have no System Auth source, a fresh Passphrase retry covers all selected software signers and does not also invoke System Auth.

System unlock does not authorize signing-material Reveal / Export.

## Backend contract

`SystemUnlockBackend` is a client interface, not a Core interface.

A backend implements:

- `available()` — whether the client can provide the facility;
- `has(slot)` — whether the exact wallet/envelope has an enrollment, without asking Core to interpret OS state;
- `enroll(slot, unlock_key)` — protect the 32-byte key under local OS policy;
- `release(slot)` — perform required OS authorization and return the 32-byte key;
- `delete(slot)` — remove the enrollment.

`SystemUnlockSlot` binds enrollment to the Stellar wallet address and a SHA-256 fingerprint of the exact canonical encrypted envelope. Re-keying or replacing the envelope therefore cannot silently reuse an old unlock key.

Backends must not store a mnemonic, Stellar `S...` secret, or Fresnica app passcode.

## macOS

A production macOS backend should use an OS facility that cryptographically gates release of the unlock key on user presence. Do not label an ordinary always-readable Keychain item as "system authentication" merely because the process checks Touch ID first.

macOS has both the legacy file-based keychain and the data-protection keychain. Apple's data-protection `SecAccessControl` model is designed around app-like code-signing/access-group entitlements, which makes a pure unsigned command-line process materially different from a normal macOS app.

Therefore the preferred macOS implementation is the existing signed native Keychain/LocalAuthentication helper pattern (or equivalent app-style host) that returns only the wallet unlock key after successful user presence. The existing Apple `FresnicaWalletUnlockKeyStore` API already matches the backend lifecycle. A Terminal backend can use `SystemAuthSlot::storage_id()` (`public-key:envelope-fingerprint`) as its signer identifier so Passphrase re-protection cannot reuse a stale enrollment. Rust Core remains unchanged.

A shell call to `/usr/bin/security` is not an acceptable substitute for per-use user-presence protection.

## Windows

A Windows TUI/CLI backend should map the same interface to Windows platform credential/Hello facilities. It must release only the 32-byte wallet unlock key after successful local authorization and keep all DPAPI/Hello-specific behavior outside Core.

## Linux

Linux has no single universal user-presence API. A client backend may integrate with the desktop's Secret Service/keyring plus an explicit local-auth policy appropriate to that environment, or report system unlock unavailable and use the Fresnica Passphrase path.

Linux may provide this backend as a separately installed high-trust `SystemAuthProvider`, but it is **not** an ordinary `fresnica-*` command plugin and must never be auto-trusted merely because an executable appears on `PATH`. Provider registration/integrity and its local-auth guarantee belong to the platform client layer.

The backend must not claim stronger guarantees than the underlying desktop/session actually provides.

## Testing

The Rust client/Terminal architecture slice is tested with an injected fake backend:

- enrollment stores only 32 unlock-key bytes;
- system release unlocks the expected wallet;
- disabling enrollment leaves the canonical wallet envelope unchanged;
- enrollment is bound to the exact encrypted envelope;
- unavailable backends leave the existing passcode flow intact.

Platform backends require platform-specific integration tests in addition to these contract tests.

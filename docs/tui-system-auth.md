# TUI / CLI System Authorization

Status: **client path implemented; OS backends are platform-specific**.

This document applies the [Client / Rust Core Security Contract](client-core-security.md) to the current Fresnica TUI/CLI.

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

The current Python TUI uses the Python reference boundary while the production Rust binding is not yet wired into this client. The client behavior is intentionally the same so the OS adapters do not need to change when the TUI switches to Rust Core.

## TUI behavior

When a backend is available:

1. Wallet Management shows **Enable system unlock** for local software wallets.
2. Enrollment requires a fresh Fresnica app passcode.
3. Fresnica derives and identity-verifies the wallet's canonical 32-byte unlock key.
4. Only that unlock key is passed to the OS backend.
5. Later unlock requests first ask the backend to perform its local authorization and release the key.
6. The released key opens the same canonical password envelope.
7. If no enrollment exists or system authorization cannot be used, the TUI falls back to the app passcode.
8. A stale unlock key is rejected by wallet AEAD authentication and the client falls back to the passcode path.

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

Therefore the preferred macOS implementation is a small signed native client helper/agent (or packaged app-style TUI host) that owns Keychain/LocalAuthentication access and returns only the wallet unlock key after successful user presence. The TUI talks to that helper as a client adapter; Rust Core remains unchanged.

A shell call to `/usr/bin/security` is not an acceptable substitute for per-use user-presence protection.

## Windows

A Windows TUI/CLI backend should map the same interface to Windows platform credential/Hello facilities. It must release only the 32-byte wallet unlock key after successful local authorization and keep all DPAPI/Hello-specific behavior outside Core.

## Linux

Linux has no single universal user-presence API. A client backend may integrate with the desktop's Secret Service/keyring plus an explicit local-auth policy appropriate to that environment, or report system unlock unavailable and use the Fresnica passcode fallback.

The backend must not claim stronger guarantees than the underlying desktop/session actually provides.

## Testing

Cross-platform CI tests the backend contract using an injected fake backend:

- enrollment stores only 32 unlock-key bytes;
- system release unlocks the expected wallet;
- disabling enrollment leaves the canonical wallet envelope unchanged;
- enrollment is bound to the exact encrypted envelope;
- unavailable backends leave the existing passcode flow intact.

Platform backends require platform-specific integration tests in addition to these contract tests.

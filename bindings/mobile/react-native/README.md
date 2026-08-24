# React Native Core module

`FresnicaCore` is the thin React Native surface over the native software-signer authorization services.
It is intentionally smaller than the full UniFFI `MobileCoreApi`: routine React Native code must not
receive `WalletUnlockKey` bytes, native biometric `Cipher` objects, or one-shot signing sessions.

## JavaScript surface

The Android and Apple modules expose the same promise-based methods:

```text
canEnrollSystemAuth() -> boolean
hasSystemAuth(expectedSignerPublicKey) -> boolean
removeSystemAuth(expectedSignerPublicKey) -> true

enrollSystemAuth(
  envelopeJson,
  appPasscode,
  expectedSignerPublicKey,
) -> true

signWithSystemAuth(
  envelopeJson,
  expectedSignerPublicKey,
  transactionXdrBase64,
  networkPassphrase,
  reason,
) -> signedXdrBase64

signWithPasscode(
  envelopeJson,
  appPasscode,
  expectedSignerPublicKey,
  transactionXdrBase64,
  networkPassphrase,
) -> signedXdrBase64
```

Only public/reviewed transaction XDR, opaque protected signer envelopes, signer identity, network
passphrase and an explicitly entered app passcode cross JavaScript/native. The per-signer unlock key
never does.

Stable Core error categories are preserved where applicable:

- `invalid-input`
- `invalid-passcode`
- `invalid-unlock-key`
- `invalid-protected-data`
- `identity-mismatch`
- `invalid-transaction`
- `core-error`

Platform authorization adds:

- `auth-in-progress`
- `user-cancel`
- `system-auth-unavailable`
- `system-auth-not-enrolled`
- `system-auth-invalidated`
- `system-auth-failed`
- `system-auth-error`
- `native-error`

## Android / Xaman host

The AAR contains `com.fresnica.core.reactnative.FresnicaCoreModule` and
`FresnicaCorePackage`. Xaman's current `ApplicationLoader` already manually registers local
`ReactPackage` implementations, so the integration is one additional package registration next to
its existing `SecurityPackage`.

The module owns `BiometricPrompt` and authenticates the exact enrollment/decrypt `Cipher` created by
`WalletUnlockKeyStore`. On prompt cancellation it explicitly destroys the pending native session.

## Apple / Xaman host

`FresnicaCoreModule.swift` owns the high-level native service. `FresnicaCoreModule.m` contains only
`RCT_EXTERN_MODULE` declarations, matching Xaman's existing Objective-C React Native bridge style.
Add both files, `FresnicaSignerAuthorization.swift`, `FresnicaWalletUnlockKeyStore.swift`, the
generated `FresnicaCore.swift`, and `FresnicaCoreFFI.xcframework` to the application target.

Keychain access performs the actual Face ID / Touch ID operation that releases a per-signer
`WalletUnlockKey`; a separate biometric-success flag is never treated as signing authorization.

## Boundary

This module is for protected local software signing. Hardware/external signers continue to use the
Core `prepare_ed25519_signing` / provider / `apply_ed25519_signature` flow and should get their own
provider orchestration rather than being forced through this module's Keychain/Keystore path.

# Mobile System Authentication / WalletUnlockKey Contract

Status: **accepted pre-release native security direction**.

This document defines how Fresnica mobile stores and releases the per-signer 32-byte `WalletUnlockKey` produced by Rust Core.

It refines the higher-level rules in `mobile-core-contract.md` and deliberately replaces Xaman's wallet-secret Vault behavior for routine Fresnica signing.

## Principle

System authentication must authorize access to the **actual WalletUnlockKey cryptographic operation**.

A separate successful Face ID / Touch ID / Android biometric probe followed by an unrelated key read is not sufficient.

```text
Core derive_unlock_key
        |
32-byte WalletUnlockKey
        |
native user-auth-bound storage
        |
biometric-gated cryptographic operation
        |
32-byte WalletUnlockKey in native memory only
        |
UniFFI sign_transaction_xdr
        |
zero/drop temporary unlock-key bytes
```

The unlock key MUST NOT cross into React Native / JavaScript during routine signing.

## Enrollment

Enrollment is an explicit convenience-security action after the user has proved the Fresnica app passcode.

```text
app passcode
   |
native -> Rust derive_unlock_key(envelope, passcode, expected_signer_public_key)
   |
verified 32-byte WalletUnlockKey
   |
platform secure storage enrollment
```

The platform record is bound to a signer identity, not an account address. In the normal master-key case those values are equal; additional/multisig signers may differ from the account identity.

The client should persist ordinary public enrollment metadata with the signer record, for example whether system auth was enrolled and which local signer it belongs to. The secure-storage item itself is authoritative at use time.

## Routine signing

```text
React Native requests a reviewed sign action
        |
native module resolves account + signer
        |
native opens system-auth WalletUnlockKey record
        |
platform prompt authenticates actual key access
        |
native obtains 32 bytes
        |
Fresnica Native SDK `FresnicaSdkApi.signTransactionXdr(...)`
        |
native clears/drops 32-byte temporary buffer
        |
signed XDR -> React Native
```

React Native may receive the signed XDR. It does not receive `WalletUnlockKey`, mnemonic, or `S...` material.

## App-passcode fallback

System authentication is optional convenience, not the recovery root.

If biometric enrollment is unavailable, invalidated, cancelled, or removed, the user can enter the Fresnica app passcode. Native code calls Core to derive a fresh verified unlock key and can either:

- use it once for signing and immediately drop it; or
- explicitly re-enroll system authentication.

A platform-auth failure must not silently fall back to exposing secret material.

## Passcode rotation

`reprotect` normally creates a new envelope salt and therefore a different `WalletUnlockKey`.

After successful passcode rotation:

1. old system-auth unlock-key records are stale;
2. the client must invalidate/delete them;
3. re-enrollment derives the new unlock key from the new envelope and new app passcode.

For a global app-passcode rotation across several protected signers, Mobile still owns staging/atomic persistence of the new envelopes. Secure-storage re-enrollment occurs only after the new envelope set is committed.

## Android

Implementation: `bindings/native/platform/android/src/main/kotlin/com/fresnica/sdk/security/WalletUnlockKeyStore.kt`.

The first Fresnica Android policy is **strong biometric, auth-per-use** with app-passcode fallback.

Each enrollment creates a new per-signer AndroidKeyStore AES-256-GCM key:

- `PURPOSE_ENCRYPT | PURPOSE_DECRYPT`;
- GCM / no padding;
- randomized encryption required;
- `setUserAuthenticationRequired(true)`;
- authentication required for every operation;
- strong biometric authentication;
- key invalidated by biometric enrollment changes;
- StrongBox requested on supported devices, with normal AndroidKeyStore/TEE fallback.

Only AES-GCM IV + ciphertext + opaque Keystore alias are kept in app-private SharedPreferences. The AES key remains non-exportable in AndroidKeyStore.

### Android enrollment transaction

Enrollment uses a fresh pending Keystore alias instead of deleting the current enrollment first:

```text
beginEnrollment
  -> create fresh auth-bound AES key
  -> return initialized ENCRYPT Cipher

BiometricPrompt.authenticate(CryptoObject(cipher))

finishEnrollment
  -> cipher.doFinal(WalletUnlockKey)
  -> commit IV/ciphertext/new alias
  -> only then delete previous alias
```

If authentication is cancelled or persistence fails, the new pending alias is deleted and the previous enrollment remains intact.

### Android unlock

```text
beginUnlock
  -> load IV/ciphertext + non-exportable AES key
  -> return initialized DECRYPT Cipher

BiometricPrompt.authenticate(CryptoObject(cipher))

finishUnlock
  -> cipher.doFinal(ciphertext)
  -> verify exactly 32 bytes
  -> caller passes bytes directly to generated Fresnica Core API
```

This is intentionally different from Xaman's current biometric `SecurityProvider`, which proves a separate biometric key can be used but does not protect the wallet credential being released.

The existing Xaman StrongBox/AndroidKeyStore fallback patterns are useful infrastructure, but the old Vault crypto and cleartext-to-React-Native path are not Fresnica signer authority.

## Apple

Implementation: `bindings/native/platform/apple/FresnicaWalletUnlockKeyStore.swift`.

Each signer unlock key is stored as a Keychain generic-password data item with:

- `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`;
- `SecAccessControlCreateFlags.biometryCurrentSet`;
- no synchronization/migration to another device.

`biometryCurrentSet` means an enrolled unlock-key record becomes unusable when Touch ID fingerprints are added/removed or Face ID is re-enrolled. Fresnica then uses app-passcode recovery/re-enrollment rather than accepting the changed biometric set automatically.

Reading the Keychain item uses a fresh `LAContext` attached through `kSecUseAuthenticationContext`. The Keychain access itself causes the system authentication that releases the 32-byte value.

This is intentionally stronger than using Xaman's existing biometric module only as a separate authentication probe.

## Xaman code that may be retained

Useful platform infrastructure includes:

- React Native module/lifecycle patterns;
- biometric availability/error UX;
- Keychain/AndroidKeyStore plumbing;
- StrongBox fallback concepts;
- Realm storage-encryption-key management;
- platform queues and background/snapshot lifecycle behavior.

The following Xaman responsibilities must not remain authoritative for Fresnica software signers:

- encrypting/decrypting mnemonic or private-key cleartext in the native Vault;
- returning routine wallet secret cleartext to React Native;
- using a global app passcode as the normal native decrypt-all-wallets signing credential;
- treating a separate biometric-probe success as equivalent to release of the actual signer credential.

## Native module API direction

The later React Native adapter should expose high-level operations rather than raw secure-storage primitives.

Native-only operations conceptually include:

```text
enrollSystemAuth(signer, envelope, app_passcode)
removeSystemAuth(signer)
hasSystemAuth(signer)

signWithSystemAuth(
  signer,
  envelope,
  transaction_xdr,
  network_passphrase,
  reason,
) -> signed_xdr

signWithPasscode(
  signer,
  envelope,
  app_passcode,
  transaction_xdr,
  network_passphrase,
) -> signed_xdr
```

Neither sign operation returns `WalletUnlockKey` to JavaScript.

Secret import/generation/reveal remain separate exceptional paths governed by `mobile-core-contract.md` and `secret-export.md`.

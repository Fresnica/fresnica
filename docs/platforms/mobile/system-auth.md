# Mobile System Authentication / WalletUnlockKey Contract

Status: **accepted Native SDK v0.2 security contract**.

This document defines the Fresnica Mobile system-authentication model for local protected software signers.

The authority order is:

```text
Fresnica app passcode > device system authentication
```

System authentication is a device-local authorization convenience for routine signing. It is not the Fresnica passcode, not a recovery credential, and not sufficient for Reveal / Export or passcode rotation.

## Core invariants

1. Each protected software signer keeps its own canonical Core envelope, random salt/nonce and 32-byte `WalletUnlockKey`.
2. A device installation has at most one Fresnica **System Auth Protection Domain**.
3. The protection domain owns one user-auth-bound private wrapping key and a public wrapping key.
4. New signer registration uses the public wrapping key and therefore does **not** require another biometric prompt.
5. Routine signing uses the auth-bound private key; the private-key operation is what triggers Face ID / Touch ID / Android strong biometric authentication.
6. `WalletUnlockKey` stays in native memory and never crosses into React Native / JavaScript.
7. No global Vault Master Key is introduced. Sharing the OS wrapping domain does not merge the independent Core signer envelopes or unlock keys.

Conceptually:

```text
                       one device System Auth Domain
                                  |
                     auth-bound private wrapping key
                     public wrapping key
                         /        |        \
                        /         |         \
              wrapped key A  wrapped key B  wrapped key C
                    |            |            |
              UnlockKey A   UnlockKey B   UnlockKey C
                    |            |            |
              envelope A    envelope B    envelope C
```

## First-time initialization

The user first establishes the Fresnica app passcode through normal wallet provisioning. System auth is optional and initialized once per device installation:

```text
Mobile -> canUseSystemAuth()
Mobile -> initializeSystemAuth(reason)
                 |
                 v
native creates pending auth-bound device private key
                 |
Face ID / Touch ID / Android strong biometric
                 |
authenticated private-key challenge succeeds
                 |
commit System Auth Protection Domain
```

The biometric prompt proves that the newly created private key is actually protected by the configured platform policy before the domain becomes active.

This operation is device-level. It is not repeated for every signer.

## Registering a signer after the domain exists

A new/imported/derived protected software signer is registered after Mobile has persisted its signer envelope and the user has proved the Fresnica passcode:

```text
protected envelope
+ Fresnica app passcode
+ expected signer public key
        |
        v
Native SDK / Core derives and verifies WalletUnlockKey
        |
        v
native uses domain PUBLIC key to wrap 32-byte WalletUnlockKey
        |
        v
store signer -> wrapped-key record
```

No Face ID / fingerprint prompt occurs here. Public-key wrapping does not require access to the auth-bound private key.

The wrapped-key record is bound to the signer identity, not the account address. `Account != Signer`; one signer may be referenced by more than one account.

If no system-auth domain exists, wallet creation/import still succeeds. The user can sign with the Fresnica passcode and may initialize system auth later.

## Routine signing with system auth

```text
React Native requests a reviewed sign action
        |
native resolves signer + protected envelope
        |
load wrapped WalletUnlockKey for signer
        |
platform prompt authorizes PRIVATE-key unwrap
        |
32-byte WalletUnlockKey exists in native memory only
        |
Native SDK signs canonical envelope/XDR for expected signer + network
        |
zero/drop temporary unlock-key bytes
        |
signed XDR -> React Native
```

System auth authorizes use of that signer for the reviewed action. React Native may receive signed XDR; it does not receive `WalletUnlockKey`, mnemonic, `S...`, platform private keys, or biometric crypto objects.

## Passcode signing fallback

System auth is optional convenience, not the recovery root.

When system auth is unavailable, invalidated, cancelled or intentionally bypassed, Mobile may ask for the Fresnica app passcode and call the native high-level passcode signing path:

```text
signWithPasscode(
  signer,
  envelope,
  app_passcode,
  transaction_xdr,
  network_passphrase,
) -> signed_xdr
```

The passcode path derives a fresh verified unlock key, signs, and drops temporary key material. It does not require a system-auth domain.

A system-auth failure must never silently expose secret material or be treated as proof of the Fresnica passcode.

## Privilege boundary

System auth MAY authorize routine signing.

System auth MUST NOT by itself authorize:

- Reveal / Export of mnemonic or `S...` material;
- changing the Fresnica app passcode;
- turning a lost passcode into a new recovery credential;
- bypassing Core signer-identity checks;
- provisioning a new signer without proving the Fresnica passcode when signer registration is requested.

Reveal / Export continues to require a fresh Fresnica app passcode. A user who can still sign with Face ID but has forgotten the Fresnica passcode has **not** regained the recovery/declassification authority of that passcode.

## Passcode rotation

`reprotect` normally creates a new envelope salt, so every affected software signer receives a new `WalletUnlockKey`.

Global passcode rotation is Mobile-coordinated and atomic at the persistence boundary:

```text
1. Snapshot every protected software signer.
2. Core.reprotect(old envelope, old passcode, new passcode, signer) for EVERY signer.
3. If any reprotect fails: write nothing.
4. Stage all returned envelopes and identities.
5. In one Realm/database transaction, re-check snapshots and replace all envelopes.
6. Commit.
7. Mark every pre-rotation signer registration stale/unavailable for the new envelope set.
8. For each committed new envelope, derive the new verified WalletUnlockKey with the new passcode.
9. Wrap/register it with the EXISTING device System Auth Domain public key.
```

Steps 1-6 are the atomic security boundary. No wallet may be left silently split between old and new app passcodes.

Step 7 is a Mobile orchestration invariant: an old wrapped key must not be presented as usable after its envelope changed, even if OS cleanup of the old record still needs retry. Step 9 does not require another biometric prompt because the protection-domain public key performs wrapping. Registration failure after the database commit is retryable; the affected signer remains system-auth unavailable and falls back to passcode signing, but the persisted envelopes must not roll back to the old passcode.

If the system-auth domain itself was invalidated or deleted, Mobile may ask the user to initialize a new domain once, then register all signers into it after passcode verification.

## Android implementation

Implementation:

- `bindings/native/platform/android/src/main/java/com/fresnica/sdk/security/WalletUnlockKeyStore.java`
- `bindings/native/platform/android/src/main/kotlin/com/fresnica/sdk/security/FresnicaSignerAuthorization.kt`

The v0.2 Android domain uses one AndroidKeyStore RSA-2048 key pair:

- private key: `PURPOSE_DECRYPT`;
- RSA OAEP SHA-256 wrapping;
- `setUserAuthenticationRequired(true)`;
- authentication required for every private-key operation;
- strong biometric authentication;
- invalidated by biometric-enrollment changes;
- StrongBox requested where supported, with AndroidKeyStore/TEE fallback.

Domain initialization performs an authenticated challenge decrypt before committing the domain.

Later signer registration encrypts the 32-byte `WalletUnlockKey` with the public key and stores only domain alias + ciphertext in app-private preferences. It does not invoke the private key and therefore does not prompt for biometrics.

Routine signing calls `beginUnlock`, authenticates the returned `BiometricPrompt.CryptoObject(Cipher)`, then `finishUnlock` obtains exactly 32 bytes for immediate Native SDK signing.

## Apple implementation

Implementation:

- `bindings/native/platform/apple/FresnicaWalletUnlockKeyStore.swift`
- `bindings/native/platform/apple/FresnicaSignerAuthorization.swift`

The v0.2 Apple domain uses one P-256 private key protected by Keychain/Security access control:

- `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`;
- `biometryCurrentSet`;
- `privateKeyUsage`;
- no synchronization or migration to another device.

Per-signer `WalletUnlockKey` values are wrapped with the public key using ECIES and stored as ciphertext records. Public-key wrapping does not require Face ID/Touch ID.

Routine unwrap uses the protected private key with a fresh `LAContext`; the private-key operation triggers the system authentication that releases the 32-byte key into native memory for immediate signing.

Changing the enrolled biometric set invalidates the domain private key. Fresnica then falls back to the app passcode and may initialize a replacement domain once.

## React Native high-level API

The framework adapter exposes high-level operations rather than raw secure-storage primitives:

```text
canUseSystemAuth()
hasSystemAuthDomain()
initializeSystemAuth(reason)

registerSignerSystemAuth(
  envelope,
  app_passcode,
  expected_signer_public_key,
)

hasSignerSystemAuth(expected_signer_public_key)
removeSignerSystemAuth(expected_signer_public_key)
removeSystemAuthDomain()

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

Neither signing path returns `WalletUnlockKey` to JavaScript.

## Xaman-derived Mobile rule

Fresnica may retain Xaman's proven platform/UI patterns such as biometric availability UX, Keychain/AndroidKeyStore plumbing, StrongBox fallback concepts, Realm lifecycle, queues and background/snapshot handling.

It must not retain Xaman wallet-secret authority that:

- decrypts mnemonic/private-key cleartext into React Native for routine signing;
- treats a global PIN/password as a native decrypt-all-wallets vault credential;
- treats a separate biometric probe as equivalent to authorization of the actual signing credential.

The final rule is:

> **Passcode establishes and changes signer protection; system auth authorizes device-local routine signer use.**

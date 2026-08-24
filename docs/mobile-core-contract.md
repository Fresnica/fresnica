# Mobile / Rust Core Vault Contract

Status: **accepted target architecture for the pre-release integration phase**.

This document maps the cross-client security contract onto `fresnica-mobile`. The normative cross-client boundary is [Client / Rust Core Security Contract](client-core-security.md).

Because Fresnica has not had a public wallet release yet, this phase does **not** require compatibility migration code for internal test data. Once a public release exists, every persisted-format change must include an explicit migration and rollback strategy.

## 1. Design goals

1. Users set one Fresnica app passcode for ordinary software wallets.
2. Every software wallet remains cryptographically independent through its own random salt and nonce.
3. Rust Core is authoritative for wallet secret formats, cryptography, derivation, unlock-key semantics, identity checks, signer construction, and transaction signing.
4. Mobile is authoritative for persistence, Keychain / Keystore use, biometrics, application lock state, and platform lifecycle.
5. System authentication is client-side signer authorization, not a Core protection provider and not a second wallet encryption format.
6. Normal software-wallet signing crosses the Mobile/Core boundary with a standard `WalletUnlockKey`, not a passcode or private key.
7. Reveal / Export crosses the boundary with a fresh app passcode and is intentionally unavailable through `WalletUnlockKey`.
8. Plaintext private signing material should cross the React Native / JavaScript boundary only for explicit user-requested Reveal / Export or unavoidable initial import.

## 2. Terminology

### App passcode

The single user-chosen Fresnica passcode used for ordinary local software-wallet protection and as the durable user-known credential for recovery and secret disclosure.

### Protected wallet envelope

A versioned encrypted blob produced and consumed by Rust Core. Mobile treats it as opaque persisted data.

For the current v1 software-wallet format, Core uses Scrypt and AES-256-GCM with random per-wallet salt and nonce.

### WalletUnlockKey

The exact 32-byte Scrypt output derived by Core from the app passcode and one wallet envelope's KDF salt.

It decrypts the same canonical password-protected wallet envelope. It does not create a second wallet ciphertext or independent system wallet key.

### System authentication

Face ID, Touch ID, Android biometric authentication, device passcode, or an equivalent platform authentication mechanism owned by Mobile/native platform code.

Biometric data is never encryption-key material and Rust Core does not invoke biometric APIs.

### Signer authorization

The client-side decision that a particular signer may be invoked now. For a local software signer, successful system authorization may release the wallet's protected `WalletUnlockKey`. For hardware/external/future signers, it may authorize provider invocation without exposing local private material.

## 3. Authoritative ownership boundary

| Concern | Rust Core | Mobile / platform |
| --- | --- | --- |
| Mnemonic / secret validation | MUST | MUST NOT duplicate |
| SEP-0005 derivation | MUST | MUST NOT duplicate |
| Wallet secret payload schema | MUST | Treat as opaque |
| Scrypt parameters and salt semantics | MUST | MUST NOT duplicate |
| `WalletUnlockKey` derivation/semantics | MUST | Store/use as opaque 32 bytes |
| AES-GCM wallet encryption/decryption | MUST | MUST NOT duplicate |
| Public-key identity validation | MUST | MAY pre-check only |
| Transaction hash/signature semantics | MUST | MUST NOT duplicate |
| External signer abstraction | MUST | Implements provider adapter |
| Persist encrypted envelope | No | MUST |
| Protect/store `WalletUnlockKey` | No | MUST |
| Realm / database storage | No | MUST |
| iOS Keychain / Secure Enclave integration | No | MUST |
| Android Keystore / StrongBox integration | No | MUST |
| Biometric UI and OS authentication | No | MUST |
| App lock/session policy | No | MUST |
| Network/UI lifecycle | No | MUST |

The central rule is:

> Core decides **what protected wallet data and unlock keys mean and how signing works**. Mobile decides **where opaque encrypted data and unlock keys are stored and when the OS authorizes their use**.

## 4. Software-wallet protection model

Fresnica does not require a global Vault Master Key for v1.

The user experience is one app passcode, while each wallet has its own salt and therefore its own unlock key:

```text
                     one Fresnica app passcode
                               |
              +----------------+----------------+
              |                |                |
          wallet A         wallet B         wallet C
          salt A           salt B           salt C
              |                |                |
           Scrypt           Scrypt           Scrypt
              |                |                |
        unlock key A     unlock key B     unlock key C
              |                |                |
        encrypted A      encrypted B      encrypted C
```

Requirements:

- Every new wallet MUST get fresh random KDF salt and AEAD nonce material.
- Core MUST own KDF and cipher parameters.
- Mobile MUST NOT implement an alternate wallet cipher.
- Mobile MUST NOT create a second biometric-specific wallet ciphertext.
- A passcode change or re-encryption with a new salt changes the wallet unlock key. Mobile MUST invalidate the old system-protected unlock-key record and enroll the new verified key.

## 5. System-auth enrollment

The credential representation is no longer an open implementation choice. The standard software-wallet credential at the Client/Core boundary is `WalletUnlockKey`.

Enrollment flow:

```text
user enters Fresnica app passcode
        |
        v
Rust Core derive_verified_unlock_key
  - derive Scrypt key from canonical envelope
  - decrypt same envelope
  - reconstruct signer
  - verify expected public key
        |
        v
WalletUnlockKey (32 bytes)
        |
        v
Mobile native layer protects/stores it
using Keychain / Keystore + platform policy
```

Core does not know how Mobile protects the resulting key.

Mobile MUST bind its stored record to the intended wallet identity and current canonical envelope/version so stale keys can be invalidated safely.

## 6. Routine signing

Target software-wallet signing flow:

```text
Mobile loads canonical envelope
        |
Mobile performs system auth or other local policy
        |
Mobile obtains WalletUnlockKey
        |
        v
Rust Core sign_protected_transaction_envelope
  - decrypt same canonical envelope
  - reconstruct signer
  - verify expected public key
  - sign exact transaction
  - drop secret-bearing signer
        |
        v
signature / signed XDR
```

Normal signing MUST NOT return mnemonic or private key material to JavaScript.

If system authentication or secure storage is unavailable, Mobile may ask for the app passcode and call `derive_verified_unlock_key` for a fresh one-shot or re-enrolled key. The canonical wallet envelope remains unchanged.

## 7. Reveal / Export

Reveal / Export is a separate declassification operation.

```text
user explicitly requests Reveal / Export
        |
fresh Fresnica app passcode
        |
        v
Rust Core export_signing_material
  - decrypt canonical envelope
  - reconstruct signer
  - verify expected public key
        |
        v
original mnemonic / S... material
```

A `WalletUnlockKey`, Face ID success, or an already-unlocked Mobile session MUST NOT be sufficient to invoke this path.

See [Signing Material Reveal and Export](secret-export.md).

## 8. Persistence model

A protected wallet envelope does not need to live directly in Keychain or Android Keystore.

Recommended separation:

```text
Realm / file / app database
    -> public wallet metadata
    -> Core protected wallet envelope

OS secure storage
    -> Realm/database encryption key
    -> per-wallet WalletUnlockKey records
    -> native authentication keys / policy
```

The Core envelope is already authenticated ciphertext. Additional platform storage encryption is defense in depth and does not become the canonical wallet format.

## 9. Core API and FFI standards

Mobile integration should expose narrow, purpose-specific calls rather than a generic `Vault.open()` equivalent.

Core-side conceptual surface:

```text
create/import + protect(passcode) -> protected envelope

derive_verified_unlock_key(envelope, passcode, expected_public_key)
    -> WalletUnlockKey

sign_protected_transaction_envelope(
    envelope,
    WalletUnlockKey,
    expected_public_key,
    transaction,
    network,
) -> signed transaction

export_signing_material(
    envelope,
    fresh_passcode,
    expected_public_key,
) -> explicit declassified material
```

Any persisted Core format MUST be explicitly versioned. Mobile MUST treat unknown versions as unsupported rather than attempting to repair or reinterpret ciphertext.

FFI errors must be mapped to stable machine-readable categories. At minimum Mobile needs to distinguish invalid passcode, invalid/stale unlock key, corrupted/unsupported protected data, wallet identity mismatch, signer/provider failure, and unsupported signing mode.

Sensitive values MUST NOT appear in logs, crash reports, analytics, or telemetry.

Rust Core continues to use zeroizing containers for derived keys and intermediate plaintext. Native adapters should avoid unnecessary copies. JavaScript strings are not suitable long-lived containers for private keys, mnemonics, passcodes, or unlock keys.

## 10. Xaman-based mobile integration

`fresnica-mobile` may retain Xaman's mature platform infrastructure:

- Keychain / Android Keystore integration;
- StrongBox / hardware-backed key support where available;
- biometric modules;
- Realm and application data persistence;
- app lock/session behavior;
- React Native UI and platform lifecycle handling.

The part that must not remain authoritative after Rust Core integration is Xaman's wallet-secret cryptography.

Target create/import flow:

```text
Mobile import UI
      |
Rust Core validate / derive / protect
      |
opaque protected wallet envelope
      |
Mobile persistence
```

Target system-auth enrollment:

```text
fresh app passcode
      |
Rust Core derive_verified_unlock_key
      |
WalletUnlockKey
      |
Mobile native secure storage
```

Target sign flow:

```text
Mobile system authentication
      |
Mobile native layer releases WalletUnlockKey
      |
Rust Core signs using same canonical envelope
```

Xaman's Realm encryption remains a mobile-storage concern and does not replace Core wallet encryption.

## 11. Mobile migration work items

Before replacing Xaman wallet signing with Rust Core, audit current native Vault call sites and classify them as:

- create/import secret;
- open/unlock secret;
- sign;
- change passcode / re-key;
- delete wallet;
- database encryption only;
- biometric/app-lock only.

Migration rule:

> Replace Xaman wallet-secret cryptography, not Xaman platform infrastructure.

The first integration should keep existing UI and storage lifecycle as stable as possible while moving wallet cryptography and signing behind the Rust Core boundary.

## 12. Public-release migration rule

There is no requirement to preserve current internal-test wallet files.

After Fresnica has a public release, every persisted Core envelope or mobile storage schema change MUST define:

1. source and destination versions;
2. authentication required to migrate;
3. identity verification after migration;
4. write-and-read-back verification;
5. atomic commit or recoverable staging;
6. rollback / recovery behavior;
7. tests using real previous-version fixtures.

Migration code must never delete the last verified readable copy before the replacement has been written and successfully reopened.

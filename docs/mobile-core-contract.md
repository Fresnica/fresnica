# Mobile / Rust Core Vault Contract

Status: **accepted target architecture for the pre-release integration phase**.

This document is the shared contract for Fresnica Core and `fresnica-mobile`. It defines ownership boundaries and code standards before the Rust Core is integrated into the mobile application.

Because Fresnica has not had a public wallet release yet, this phase does **not** require compatibility migration code for internal test data. Once a public release exists, every persisted-format change must include an explicit migration and rollback strategy.

## 1. Design goals

1. Users set one Fresnica app passcode for normal software wallets.
2. Every software wallet remains cryptographically independent through its own random salt and nonce.
3. Rust Core is authoritative for wallet secret formats, cryptography, derivation, signer construction, identity checks, and transaction signing.
4. Mobile is authoritative for persistence, Keychain / Keystore use, biometrics, application lock state, and platform lifecycle.
5. System authentication is an authorization mechanism, not a second wallet encryption format.
6. The authentication model must also work for hardware, external, remote, and future signer types that do not expose local private keys.
7. Plaintext private signing material should cross the React Native / JavaScript boundary only when unavoidable during initial import. Normal signing must not require returning a private key to JavaScript.

## 2. Terminology

### App passcode

The single user-chosen Fresnica passcode used for ordinary local software-wallet protection.

The product presents one passcode to the user. Core may use that same passcode with independent per-wallet salts, so the resulting encryption key is different for every wallet.

### Protected wallet envelope

A versioned encrypted blob produced and consumed by Rust Core. Mobile treats it as opaque persisted data.

For the current v1 software-wallet format, Core uses Scrypt and AES-256-GCM with random per-wallet salt and nonce.

### Signing material

The secret or mnemonic data required to construct a local `SoftwareSigner`.

### System authentication

Face ID, Touch ID, Android biometric authentication, Windows Hello, device passcode, or an equivalent platform authentication mechanism.

Biometric data is never encryption-key material.

### Signer authorization

The decision that a particular signer may be invoked now. For a software signer, authorization eventually permits Core to unlock its protected signing material. For an external or hardware signer, authorization permits invocation of that signer without exposing a wallet secret.

## 3. Authoritative ownership boundary

| Concern | Rust Core | Mobile / platform |
| --- | --- | --- |
| Mnemonic / secret validation | MUST | MUST NOT duplicate |
| SEP-0005 derivation | MUST | MUST NOT duplicate |
| Wallet secret payload schema | MUST | Treat as opaque |
| Scrypt parameters and salt semantics | MUST | MUST NOT duplicate |
| AES-GCM wallet encryption | MUST | MUST NOT duplicate |
| Public-key identity validation after unlock | MUST | MAY pre-check only |
| Transaction hash/signature semantics | MUST | MUST NOT duplicate |
| External signer abstraction | MUST | Implements provider adapter |
| Persist encrypted envelope | No | MUST |
| Realm / database storage | No | MUST |
| iOS Keychain / Secure Enclave integration | No | MUST |
| Android Keystore / StrongBox integration | No | MUST |
| Biometric UI and OS authentication | No | MUST |
| App lock/session policy | No | MUST |
| Network/UI lifecycle | No | MUST |

The central rule is:

> Core decides **what protected wallet data means and how it signs**. Mobile decides **where opaque encrypted data is stored and when the user is authorized to use a signer**.

## 4. Software-wallet protection model

Fresnica does not require a global Vault Master Key for v1.

The user experience is one app passcode, while each wallet remains independently encrypted:

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
          AES key A        AES key B        AES key C
              |                |                |
        encrypted A      encrypted B      encrypted C
```

Requirements:

- Every new wallet MUST get fresh random KDF salt and AEAD nonce material.
- Core MUST own KDF and cipher parameters.
- Mobile MUST NOT create an alternate implementation of the wallet cipher.
- A passcode change MAY require re-protecting local software-wallet envelopes in this model. That is acceptable for v1; once publicly released, it must use a transactional migration path.
- Optional future high-security per-wallet passphrases may be added as a product feature, but they are not the default wallet model.

## 5. System authentication model

System authentication is **not** a peer `ProtectionProvider` that creates a second encrypted copy of the wallet.

The logical flow is:

```text
system authentication
        |
        v
signer authorization
        |
        +--> software signer -> authorize Core-compatible unlock -> Core signs
        |
        +--> hardware signer -> authorize device invocation -> device signs
        |
        +--> external signer -> authorize provider invocation -> provider signs
```

For local software wallets, the mobile platform may keep Core-compatible unlock material behind Keychain / Keystore policy so a successful system authentication can avoid asking for the app passcode again. The exact native representation is an adapter concern and must not create a second wallet ciphertext format.

The following rules are fixed even before the exact FFI shape is finalized:

- The system-auth path MUST unlock the same canonical Core wallet envelope used by manual passcode entry.
- Mobile MUST NOT persist plaintext mnemonic or private key as the system-auth shortcut.
- Mobile MUST NOT create a second independently encrypted wallet payload merely for biometrics.
- Long-lived system secrets SHOULD remain in native secure storage and SHOULD NOT be exposed to the JavaScript runtime.
- A successful system authentication may establish a short-lived app session that authorizes multiple software wallets. Product policy controls session duration; it does not change the Core wallet format.
- Enrollment changes, secure-store invalidation, or platform-auth failure MUST fall back to the app passcode path rather than making the wallet unrecoverable.

### Open implementation choice before FFI freeze

The mobile team and Core team still need to choose the concrete system-unlock credential representation. Valid designs include a platform-protected Core-derived unlock key or a platform-protected app-level unlock credential. The choice must satisfy the fixed rules above and remain opaque to JavaScript where practical.

This is intentionally left as an integration decision rather than encoded into the wallet envelope format.

## 6. Persistence model

A protected wallet envelope does not need to live directly in Keychain or Android Keystore.

Recommended separation:

```text
Realm / file / app database
    -> public wallet metadata
    -> Core protected wallet envelope

OS secure storage
    -> Realm/database encryption key
    -> system-auth unlock credential or native key handle
    -> platform authentication keys / policy
```

The encrypted Core envelope may be stored in ordinary application persistence because its confidentiality and integrity are already provided by the Core envelope. The mobile application may add an additional platform storage-encryption layer, but that layer must be treated as defense in depth rather than the canonical wallet format.

## 7. Core API and FFI standards

### 7.1 Prefer one-shot secret operations

Normal mobile signing SHOULD use a one-shot Core call rather than returning plaintext signing material across FFI.

Conceptually:

```text
load encrypted envelope
        |
Core unlock + identity check + sign
        |
return signature / signed envelope
```

The existing `unlock_software_signer` identity check is a required invariant: decrypted signing material must produce the expected public key before it can be used.

### 7.2 Do not expose private keys for routine signing

After account import, mobile code SHOULD persist only public metadata and the Core protected envelope. Normal signing SHOULD NOT call an API equivalent to `Vault.open()` that returns a raw private key into JavaScript.

If plaintext material enters from an import UI, it SHOULD be handed to a native/Core boundary immediately and discarded by the UI layer as soon as the protected envelope is created.

### 7.3 Version all persisted Core formats

Any Rust Core value persisted by mobile MUST be explicitly versioned. Mobile MUST treat unknown versions as unsupported rather than attempting to interpret or repair the ciphertext.

### 7.4 Stable error contract

FFI errors MUST be mapped to stable machine-readable categories. User-visible localization belongs to mobile.

At minimum the integration must distinguish:

- invalid passcode / authentication failure;
- corrupted or unsupported protected data;
- wallet identity mismatch;
- system authentication unavailable or invalidated;
- signer/provider failure;
- unsupported transaction/signing mode.

Sensitive values MUST NOT appear in errors, crash reports, analytics, or logs.

### 7.5 Secret lifetime

Rust Core MUST continue using zeroizing containers for derived keys and intermediate plaintext buffers.

Native adapters MUST avoid unnecessary copies. JavaScript strings are not suitable long-lived containers for private keys, mnemonics, passcodes, or derived wallet keys.

## 8. Xaman-based mobile integration

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
      v
Rust Core: validate / derive / protect
      |
      v
opaque protected wallet envelope
      |
      v
Mobile persistence
```

Target sign flow:

```text
Mobile loads envelope
      |
Mobile obtains passcode or system authorization
      |
      v
Rust Core unlocks + validates identity + signs
      |
      v
signature / signed XDR
```

Xaman's Realm encryption remains a mobile-storage concern and does not replace Core wallet encryption.

## 9. Current Rust Core delta

The current Rust implementation already has the correct foundations for the password path:

- Scrypt + AES-256-GCM password envelopes;
- per-envelope random salt and nonce;
- zeroizing derived keys and plaintext buffers;
- protected signing-material parsing;
- public-key identity verification before returning a software signer;
- a signer abstraction that already supports external providers.

However, the current `SystemProtectionProvider` / `SystemKeyStore` path models system protection as a second wallet-encryption provider with a separate random wallet key. That was useful to establish the platform boundary, but it is **not the target mobile product model defined by this contract**.

Core implementation work should therefore:

1. Keep the password-protected software-wallet envelope as the canonical local wallet format.
2. Keep low-level key-based AEAD primitives only where they serve a concrete Core or adapter need; do not expose them as a second user-facing wallet format by default.
3. Decouple system authentication / signer authorization from `ProtectionCredential::System` and from the registry's mutually exclusive protection kind.
4. Preserve the protected-material -> identity validation -> signer path.
5. Add an FFI-oriented one-shot protected signing API before mobile integration so routine signing does not return private keys to JavaScript.
6. Define the native system-auth unlock credential contract with the mobile team before freezing that FFI API.

## 10. Mobile migration work items

Before replacing Xaman wallet signing with Rust Core, mobile development should audit all current call sites of native Vault APIs and classify them as:

- create/import secret;
- open/unlock secret;
- sign;
- change passcode / re-key;
- delete wallet;
- database encryption only;
- biometric/app-lock only.

Migration rule:

> Replace Xaman wallet-secret cryptography, not Xaman platform infrastructure.

The first integration should keep the existing UI and storage lifecycle as stable as possible while moving wallet cryptography and signing behind the Rust Core boundary.

## 11. Public-release migration rule

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

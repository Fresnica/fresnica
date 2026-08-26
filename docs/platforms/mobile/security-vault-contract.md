# Mobile / Rust Core Vault Contract

Status: **accepted target architecture for the pre-release integration phase**.

This document maps the cross-client security contract onto `fresnica-mobile`. The normative cross-client boundary is [Client / Rust Core Security Contract](../../core/client-security.md).

Because Fresnica has not had a public wallet release yet, this phase does **not** require compatibility migration code for internal test data. Once a public release exists, every persisted-format change must include an explicit migration and rollback strategy.

## 1. Design goals

1. Users set one Fresnica app passcode for ordinary local software signers.
2. Every protected software signer remains cryptographically independent through its own random salt and nonce.
3. Account identity, signing capability, and recovery source are separate concepts. A watch-only account is an account with no locally available signer.
4. Rust Core is authoritative for Stellar identity parsing, wallet recovery-material validation, cryptography, derivation, unlock-key semantics, signer identity checks, signing, and re-protection.
5. Mobile is authoritative for account/signer persistence, Keychain / Keystore use, biometrics, application lock state, network state, and platform lifecycle.
6. System authentication is one device/app-level authorization domain, not a Core protection provider, not a second signer encryption format, and not a replacement for the Fresnica passcode.
7. Normal software-signer signing crosses the Mobile/Core boundary with a standard `WalletUnlockKey`, not a passcode or private key.
8. Reveal / Export crosses the boundary with a fresh app passcode and is intentionally unavailable through `WalletUnlockKey`.
9. Plaintext private signing material should cross the React Native / JavaScript boundary only for explicit user-requested Reveal / Export or unavoidable initial import.

## 2. Mobile account and signer model

`fresnica-mobile` should not copy Xaman's `Account.accessLevel + encryptionLevel + publicKey` coupling as the new authoritative model.

Conceptually Mobile persists:

```text
AccountRecord
  id
  identity
    address: G... / C...
    kind: classic / contract
  network
  name / label
  product metadata

SignerRecord
  id
  signer_public_key: G...
  kind: protected-software | hardware | external | future
  envelope?: opaque Core data
  provider metadata?: client-owned

AccountSignerReference
  account_id -> signer_id

RecoverySourceRecord / grouping metadata (Mobile-owned)
  identifies a shared mnemonic recovery source for UX/backup/HD grouping
  does not replace any protected signer envelope
```

The first implementation may store this more compactly, but the semantics MUST remain separate: **Account != Signer != Recovery Source**. Recovery-source grouping is Mobile metadata; Core still protects each software signer independently.

For a simple software wallet, account address and signer public key are the same `G...`. Stellar additional signers and multisig mean this equality is not a general rule.

A `C...` contract address is not an Ed25519 signer public key. Contract/passkey authorization must not be modeled as attaching an arbitrary `S...` key directly to the `C...` identity.

## 3. Watch-only lifecycle

### Add watch-only account

```text
Mobile input: G... or C...
        |
Rust Core parse_account
  - validate StrKey
  - classify classic / contract
        |
AccountIdentity
        |
Mobile persists AccountRecord only
```

No app passcode, mnemonic, secret key, envelope, or `WalletUnlockKey` is required.

Read-only balance/history/SDEX networking remains a Mobile/network concern after identity validation.

### Upgrade classic watch-only account

For the common master-key case:

```text
existing AccountRecord GABC...
        |
user enters S... or mnemonic
        |
Core protect_secret / protect_mnemonic
  expected_signer_public_key = GABC...
        |
Core derives signer key and verifies equality
        |
ProtectedSoftwareSigner
        |
Mobile stores SignerRecord and attaches it to existing AccountRecord
```

If the secret derives a different signer, Core returns `identity-mismatch` and Mobile leaves the account unchanged.

The account is not recreated and its label, network, history, cache state, and stable client ID remain intact.

For future delegated/multisig signer attachment, `expected_signer_public_key` identifies the specific signer being imported and may differ from the account address. Mobile must use current ledger state to determine whether that signer is authorized for the account.

### Downgrade to watch-only

After explicit user authorization, Mobile removes the local signer reference and its protected material while preserving the account record:

```text
AccountRecord stays
SignerRecord / envelope removed if no longer referenced
stored WalletUnlockKey removed
```

No empty Core envelope is created.

## 4. Terminology

### App passcode

The single user-chosen Fresnica passcode used for ordinary local software-signer protection and as the durable user-known credential for recovery and secret disclosure.

### Protected software-signer envelope

A versioned encrypted blob produced and consumed by Rust Core. Mobile treats it as opaque persisted data.

For the current v1 encrypted format, Core uses Scrypt and AES-256-GCM with random per-signer salt and nonce.

### WalletUnlockKey

The exact 32-byte Scrypt output derived by Core from the app passcode and one software-signer envelope's KDF salt.

It decrypts the same canonical password-protected signer envelope. It does not create a second ciphertext or independent system wallet key.

### System authentication

Face ID, Touch ID, Android biometric authentication, device passcode, or an equivalent platform authentication mechanism owned by Mobile/native platform code.

Fresnica Mobile initializes one device-level **System Auth Protection Domain**. The domain has an auth-bound private wrapping key and a public wrapping key. New per-signer `WalletUnlockKey` values are wrapped with the public key after passcode verification, so adding another signer does not require another biometric prompt. Routine signing requires the protected private-key operation and therefore invokes system authentication.

Biometric data is never encryption-key material and Rust Core does not invoke biometric APIs. System auth is lower privilege than the Fresnica app passcode and cannot authorize Reveal / Export or passcode change.

### Signer authorization

The client-side decision that a particular signer may be invoked now. For a local software signer, successful system authorization may unwrap that signer's protected `WalletUnlockKey` through the shared device protection domain. For hardware/external/future signers, it may authorize provider invocation without exposing local private material.

## 5. Authoritative ownership boundary

| Concern | Rust Core | Mobile / platform |
| --- | --- | --- |
| `G...` / `C...` identity parsing | MUST | MAY pre-check only |
| Mnemonic / secret validation | MUST | MUST NOT duplicate |
| SEP-0005 derivation | MUST | MUST NOT duplicate |
| Signing-material payload schema | MUST | Treat as opaque |
| Scrypt parameters and salt semantics | MUST | MUST NOT duplicate |
| `WalletUnlockKey` derivation/semantics | MUST | Store/use as opaque 32 bytes |
| AES-GCM signer encryption/decryption | MUST | MUST NOT duplicate |
| Signer-public-key identity validation | MUST | MAY pre-check only |
| Transaction hash/signature semantics | MUST | MUST NOT duplicate |
| External signature verification | MUST | Provider invocation only |
| Account-to-signer authorization from ledger | Provides crypto primitives | MUST resolve/orchestrate |
| Persist AccountRecord / SignerRecord | No | MUST |
| Persist encrypted envelope | No | MUST |
| Protect/store `WalletUnlockKey` | No | MUST |
| Realm / database storage | No | MUST |
| iOS Keychain / Secure Enclave integration | No | MUST |
| Android Keystore / StrongBox integration | No | MUST |
| Biometric UI and OS authentication | No | MUST |
| App lock/session policy | No | MUST |
| Network/UI lifecycle | No | MUST |

The central rule is:

> Core decides what identities, protected signer data, unlock keys, and signatures mean. Mobile decides which account/signer records exist, where opaque data is stored, which ledger-authorized signer to invoke, and when the OS authorizes that invocation.

## 6. Software-signer protection model

Fresnica does not require a global Vault Master Key for v1.

The user experience is one app passcode, while each protected signer has its own salt and therefore its own unlock key:

```text
                     one Fresnica app passcode
                               |
              +----------------+----------------+
              |                |                |
          signer A         signer B         signer C
          salt A           salt B           salt C
              |                |                |
           Scrypt           Scrypt           Scrypt
              |                |                |
        unlock key A     unlock key B     unlock key C
              |                |                |
        encrypted A      encrypted B      encrypted C
```

Requirements:

- Every new protected signer MUST get fresh random KDF salt and AEAD nonce material.
- Core MUST own KDF and cipher parameters.
- Mobile MUST NOT implement an alternate signer cipher.
- Mobile MUST NOT create a second biometric-specific signer ciphertext.
- A passcode change or re-encryption with a new salt changes the signer unlock key. After the new envelopes are atomically committed, Mobile MUST replace each stale wrapped unlock-key record by registering the newly verified key into the existing device System Auth Protection Domain. This public-key wrapping step does not require another biometric prompt.

## 7. Create and import

### Import secret

```text
Mobile -> Core
  S... secret
  app passcode
  expected_signer_public_key?   # present for upgrade/attachment

Core -> Mobile
  signer_public_key
  protected envelope
```

### Import mnemonic

```text
Mobile -> Core
  mnemonic
  BIP39 passphrase
  index
  language
  app passcode
  expected_signer_public_key?

Core -> Mobile
  signer_public_key
  protected envelope
```

Core does not echo the mnemonic on import because Mobile already received it from the user.

### Generate mnemonic

```text
Mobile -> Core
  language
  strength
  BIP39 passphrase
  index
  app passcode

Core -> Mobile
  signer_public_key
  protected envelope
  mnemonic once
```

Mobile persists only public metadata and the opaque protected envelope. The generated mnemonic is shown only in the explicit backup/confirmation flow.

### Derive another signer from an existing mnemonic source

The default HD index is `0`. Mobile may explicitly request another index without asking the user to re-enter or Reveal the mnemonic:

```text
Mobile -> Core derive_mnemonic_signer
  source protected mnemonic-backed envelope
  app passcode
  expected source signer public key
  index N

Core
  authenticate/decrypt source envelope internally
  verify source signer identity
  derive the same mnemonic at index N
  create a fresh protected signer envelope
  keep mnemonic inside Core

Core -> Mobile
  signer_public_key
  protected envelope
```

A secret-key-backed source is rejected. The new signer remains cryptographically independent because its returned envelope receives fresh protection parameters. Mobile may group the resulting signers under one recovery-source identifier for backup/HD UX, but that grouping is not a replacement for the signer envelopes.

## 8. System-auth domain and signer registration

The credential representation is not an open implementation choice. The standard software-signer credential at the Client/Core boundary is `WalletUnlockKey`.

System auth has two distinct operations:

1. **Initialize the device protection domain once**. This creates an auth-bound platform private wrapping key and proves its biometric/system-auth policy with one authenticated challenge.
2. **Register each signer**. After the user proves the Fresnica app passcode, Core derives and verifies that signer's 32-byte `WalletUnlockKey`; native code wraps it with the already-existing domain public key. Signer registration does not require another biometric prompt.

```text
initializeSystemAuth(reason)
  -> one device-level system-auth prompt
  -> commit System Auth Protection Domain

registerSignerSystemAuth(envelope, app_passcode, signer)
  -> Core derives/verifies WalletUnlockKey
  -> domain public key wraps it
  -> persist signer -> wrapped-key record
  -> no biometric prompt
```

Core does not know how Mobile protects the resulting key. Mobile MUST bind each wrapped record to the intended signer identity and current domain, and it MUST treat a missing/invalid domain as a passcode-signing fallback rather than a recovery failure.

## 9. Routine signing

Target local software-signer flow:

```text
Mobile resolves account authorization and selects signer
        |
Mobile loads signer envelope
        |
Mobile performs system auth or other local policy
        |
Mobile obtains signer WalletUnlockKey
        |
        v
Rust Core sign_protected_transaction_envelope
  - decrypt same canonical envelope
  - reconstruct signer
  - verify expected signer public key
  - sign exact transaction
  - drop secret-bearing signer
        |
        v
signed XDR
```

Normal signing MUST NOT return mnemonic or private key material to JavaScript.

If system authentication or secure storage is unavailable, Mobile may ask for the app passcode and call `derive_verified_unlock_key` for a fresh one-shot or re-enrolled key. The canonical signer envelope remains unchanged.

## 10. Passcode change / re-protection

Mobile MUST NOT implement passcode change as `Reveal -> encrypt again`.

Core exposes a dedicated re-protection operation:

```text
reprotect(
  envelope,
  current_passcode,
  new_passcode,
  expected_signer_public_key,
) -> new protected software signer
```

Plaintext recovery material remains inside Core.

Because Fresnica presents one app passcode across ordinary software signers, a global passcode change is a Mobile-coordinated batch:

1. snapshot every affected protected signer;
2. call Core `reprotect` for **every** signer using old + new passcodes;
3. verify every returned signer identity and stage every new envelope;
4. if any call fails, write nothing;
5. in one Realm/database transaction, re-check the snapshots and replace all envelopes;
6. commit the envelope set atomically;
7. mark every previous signer system-auth registration stale for the new envelope generation;
8. derive each new verified `WalletUnlockKey` from the committed envelope + new passcode;
9. register/replace each wrapped key using the **existing System Auth Protection Domain public key**.

Steps 1-6 are the atomic protection transition. Failure of one signer MUST NOT leave a silently mixed old/new app-passcode state. Step 7 is application orchestration: an old wrapped key cannot be treated as current merely because an OS record still exists. Step 9 requires no biometric prompt and may be retried after commit; a temporary registration failure leaves that signer system-auth unavailable and falls back to passcode signing rather than rolling envelopes back. If the domain itself was invalidated, Mobile initializes a replacement domain once and then registers all signers.

## 11. Reveal / Export

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
  - verify expected signer public key
        |
        v
original mnemonic / S... material
```

A `WalletUnlockKey`, Face ID success, or an already-unlocked Mobile session MUST NOT be sufficient to invoke this path.

See [Signing Material Reveal and Export](../../core/secret-export.md).

## 12. External / hardware signer flow

Xaman's Tangem path demonstrates a separate class of signer: Mobile invokes a device/provider that owns the private key. This must not be represented as a password-protected Core envelope.

Target transport-neutral flow:

```text
Mobile -> Core prepare_ed25519_signing
        |
transaction_hash + public signing context
        |
Mobile invokes Tangem / Ledger / other provider
        |
64-byte Ed25519 signature
        |
Mobile -> Core apply_ed25519_signature
        |
Core recomputes hash, verifies signature and signer key
        |
signed XDR
```

This avoids FFI callbacks while keeping transaction-hash and signature-validation semantics authoritative in Core.

## 13. Persistence model

Recommended separation:

```text
Realm / file / app database
    -> AccountRecord
    -> SignerRecord metadata
    -> Core protected software-signer envelope
    -> account-to-signer references

OS secure storage
    -> Realm/database encryption key
    -> one device System Auth Protection Domain private key
    -> per-signer wrapped WalletUnlockKey records
    -> native authentication policy
```

The Core envelope is already authenticated ciphertext. Additional platform storage encryption is defense in depth and does not become the canonical signer format.

## 14. Core API and FFI standards

Mobile integration should expose narrow, purpose-specific calls rather than a generic `Vault.open()` equivalent.

Core-side conceptual surface:

```text
parse_account(address) -> AccountIdentity

protect_secret(..., expected_signer_public_key?)
    -> ProtectedSoftwareSigner

protect_mnemonic(..., expected_signer_public_key?)
    -> ProtectedSoftwareSigner

generate_mnemonic(...)
    -> ProtectedSoftwareSigner + one-time mnemonic

derive_mnemonic_signer(
    source_envelope, passcode, expected_source_signer_public_key, index
) -> ProtectedSoftwareSigner

reprotect(envelope, old_passcode, new_passcode, expected_signer_public_key)
    -> ProtectedSoftwareSigner

derive_verified_unlock_key(envelope, passcode, expected_signer_public_key)
    -> WalletUnlockKey

validate_unlock_key(envelope, WalletUnlockKey, expected_signer_public_key)

sign_protected_transaction_envelope(
    envelope,
    WalletUnlockKey,
    expected_signer_public_key,
    transaction,
    network,
) -> signed transaction

export_signing_material(
    envelope,
    fresh_passcode,
    expected_signer_public_key,
) -> explicit declassified material

prepare_ed25519_signing(...) -> public signing request
apply_ed25519_signature(...) -> signed transaction
```

Any persisted Core format MUST be explicitly versioned. Mobile MUST treat unknown versions as unsupported rather than attempting to repair or reinterpret ciphertext.

FFI errors must be mapped to stable machine-readable categories. At minimum Mobile needs to distinguish invalid input/address, invalid passcode, invalid/stale unlock key, corrupted/unsupported protected data, signer identity mismatch, invalid transaction/signature, signer/provider failure, and unsupported signing mode.

Sensitive values MUST NOT appear in logs, crash reports, analytics, or telemetry.

Rust Core continues to use zeroizing containers for derived keys and intermediate plaintext. Native adapters should avoid unnecessary copies. JavaScript strings are not suitable long-lived containers for private keys, mnemonics, passcodes, or unlock keys.

## 15. Xaman-derived mobile integration

`fresnica-mobile` may retain Xaman's mature platform infrastructure:

- Keychain / Android Keystore integration;
- StrongBox / hardware-backed key support where available;
- biometric modules;
- Realm and application data persistence;
- app lock/session behavior;
- React Native UI and platform lifecycle handling.

The part that must not remain authoritative after Rust Core integration is Xaman's wallet-secret cryptography or its assumption that one Account model owns its private key/encryption state.

Xaman interaction patterns that Fresnica should preserve semantically include:

- add read-only account;
- read-only -> signing-capable upgrade after secret identity verification;
- signing-capable -> read-only downgrade without recreating the account;
- PIN / biometric authorization before local signing;
- account passcode/security changes;
- hardware/external signer invocation.

Fresnica deliberately changes the underlying implementation:

- account identity and signer records are separate;
- one device System Auth Protection Domain unwraps per-signer `WalletUnlockKey` values for routine signing; new signer registration uses the domain public key and does not repeat biometric enrollment;
- passcode rotation uses Core `reprotect`, not Mobile plaintext Vault operations;
- external signers use Core-prepared/verified signing requests rather than Core owning device secrets.

Migration rule:

> Replace Xaman wallet-secret cryptography and account/private-key coupling, not Xaman platform infrastructure or proven interaction flows.

## 16. Public-release migration rule

There is no requirement to preserve current internal-test wallet files.

After Fresnica has a public release, every persisted Core envelope or mobile storage schema change MUST define:

1. source and destination versions;
2. authentication required to migrate;
3. signer identity verification after migration;
4. write-and-read-back verification;
5. atomic commit or recoverable staging;
6. rollback / recovery behavior;
7. tests using real previous-version fixtures.

Migration code must never delete the last verified readable copy before the replacement has been written and successfully reopened.

# Software Signer Protection Model

Fresnica separates account identity, signing capability, recovery source, local secret material, Core protection, client persistence, and system authentication.

```text
Account Identity
      |
zero or more authorized Signers
      |
Protected Software Signer (local software only)
      |
Secret / Mnemonic Material
      |
Core Protection
      |
Canonical Encrypted Signer Envelope
      |
Client Persistence
```

System authentication sits beside this flow as **client-side signer authorization**. It is not a Core protection provider and it is not a wallet encryption algorithm.

See [Client / Core Security Contract](client-security.md) for the cross-client boundary and [Mobile / Rust Core Vault Contract](../platforms/mobile/security-vault-contract.md) for the mobile mapping.

## Fresnica passphrase and local software signers

The default product model uses one Fresnica passphrase for ordinary local software signers.

The user sees one passphrase, but each signer envelope has its own random Scrypt salt and AES-GCM nonce. Therefore the same passphrase produces a different 32-byte unlock key for each protected signer.

`PasswordProtectionProvider` retains the version-1 Scrypt + AES-256-GCM envelope behavior.

The Core owns:

- signing-material payload format;
- KDF parameters and salt semantics;
- AEAD encryption/decryption;
- unlock-key derivation from passphrase + envelope KDF metadata;
- secret parsing and zeroization;
- signer identity verification before an unlock key is enrolled or used;
- re-protection without exporting plaintext signing material.

Clients must not duplicate those rules.

### New-protection passphrase floor

Fresnica product/application policy requires a passphrase of at least 15 Unicode scalar values when creating a new protected software signer or rotating/re-protecting one. This is a Wallet/Application credential policy, not a new Core protection format: Core continues to own KDF/encryption semantics and accepts the credential bytes supplied by its trusted caller.

The baseline rejects short PIN-style credentials without requiring uppercase/lowercase/digit/symbol mixtures. Products must not silently normalize the passphrase before passing it to Core. The native Rust CLI enforces this reference policy; Mobile and other products must enforce the same policy in their onboarding and protection-settings flows.

The minimum applies only when establishing new protection. Existing envelopes remain unlockable with their original credential so a user can authenticate an older weak envelope and rotate it to a compliant passphrase instead of being locked out.

The current version-1 Scrypt parameters remain unchanged in this hardening step. A KDF-cost increase must be benchmarked on supported iOS/Android hardware and introduced as an explicit versioned envelope migration rather than silently changing the meaning of version 1.

Historical public API names such as `appPasscode`, `signWithPasscode` and `invalid-passcode` remain compatibility names until a later API version; they refer to the Fresnica passphrase credential.

## Standard software-signer unlock key

For a password-protected software signer, `WalletUnlockKey` is the exact 32-byte Scrypt output that encrypts that signer's canonical password envelope. The historical type name is retained because it is already part of the Core vocabulary; semantically the key is scoped to one protected software-signer envelope.

```text
Fresnica Passphrase + signer salt
          |
        Scrypt
          |
  WalletUnlockKey (32 bytes)
          |
     AES-256-GCM
          |
canonical signer envelope
```

Core exposes a redacted `WalletUnlockKey` type and a verified derivation path. The verified path derives the key, opens the canonical envelope, reconstructs the signer, and checks `expected_signer_public_key` before returning the unlock key to a client for enrollment.

No second ciphertext and no independent system wallet key are created.

## Account, signer, and recovery source are distinct

For a simple Stellar master-key wallet, the account address and signer public key are the same `G...` value. This is not a generic invariant.

Stellar accounts can authorize additional signer keys and multisig combinations, so a protected signer envelope carries a signer identity, not an account identity. Whether that signer is currently authorized for an account is determined from ledger state and client policy outside the local envelope.

A watch-only account has no local signer envelope. Adding a matching secret/mnemonic later attaches a protected signer to the existing account record; it does not create a new account.

A mnemonic recovery source may derive several signer identities at different explicit indices. Mobile may group those signers for backup/HD UX, but each signer still owns a fresh independent protected envelope. The Core v0.2 `derive_mnemonic_signer` path authenticates an existing mnemonic-backed envelope and derives another index internally so the mnemonic does not need to cross the Core boundary again. Therefore **Account != Signer != Recovery Source**.

## System authentication

Face ID, Touch ID, Android biometrics, Windows Hello, device passcode, PAM-backed local policy, or another operating-system mechanism belongs entirely to the client/platform layer.

The client decides how to protect and release the 32-byte `WalletUnlockKey`. Core does not know which mechanism was used. Fresnica Mobile v0.2 uses one device/app-level System Auth Protection Domain: one auth-bound private wrapping key plus a public wrapping key. After the Fresnica passphrase verifies a new signer, the public key wraps that signer's independent `WalletUnlockKey` without another biometric prompt. Routine signing requires the private-key unwrap and therefore invokes system authentication.

This does **not** create a global wallet master key. The domain only protects independent per-signer unlock keys; Core envelopes and KDF salts remain per signer. System auth is lower privilege than the Fresnica passphrase and cannot authorize Reveal / Export or passphrase change.

Normal software-signer signing is therefore:

```text
Client resolves/selects signer
        |
Client OS authentication
        |
client releases signer WalletUnlockKey
        |
        v
Rust Core
same canonical signer envelope
  -> decrypt
  -> verify signer identity
  -> sign
  -> drop secret-bearing signer
```

If system authentication becomes unavailable, the client can ask the user for the Fresnica passphrase, derive a fresh verified unlock key through Core, and continue using the same canonical envelope.

If the passphrase or canonical envelope is re-protected with a new salt, the old wrapped key record becomes stale. After the new envelope set is committed, Mobile derives each new verified unlock key and wraps it with the existing System Auth Domain public key. This registration step does not require another biometric prompt.

## Passphrase rotation / re-protection

Changing protection MUST happen inside Core:

```text
old envelope + current passphrase + new passphrase + expected signer
        |
Core decrypts and verifies signer
        |
Core encrypts same recovery material with fresh parameters
        |
new envelope
```

Clients MUST NOT implement password change as `Reveal -> encrypt`, because that unnecessarily crosses the declassification boundary.

When one Fresnica passphrase protects several local software signers, the client MUST stage every per-signer `reprotect`, atomically commit the complete new envelope set, immediately treat every previous system-auth registration as stale for that new envelope generation, then replace each wrapped unlock-key record in the existing System Auth Protection Domain. Any pre-commit failure writes nothing; post-commit registration failures are retryable and leave the affected signer on passphrase signing until registration succeeds.

## OS boundary

Core MUST NOT implement or abstract platform authentication APIs. In particular, Core does not own:

- Keychain / Secure Enclave APIs;
- Android Keystore / StrongBox APIs;
- Windows Hello / Credential Manager / DPAPI policy;
- Linux Secret Service, PAM, desktop keyrings, or equivalent mechanisms;
- biometric prompts, app sessions, or platform authorization UX.

Different clients may implement different OS adapters. They converge only at the Core boundary: a valid `WalletUnlockKey` for normal protected-software signing.

## External signer boundary

Hardware and external signers do not use local software-signer unlock keys. The same client-side authorization concept may instead permit invocation of a hardware, external, remote, or future contract/passkey signer.

For external Ed25519 signers, Core prepares the exact public signing request and later verifies/applies the returned signature. The provider owns its private material; no password envelope or `WalletUnlockKey` is manufactured for it.

Reveal / Export is intentionally different: it accepts a fresh Fresnica passphrase, not a `WalletUnlockKey`. See [Signing Material Reveal and Export](secret-export.md).

## Public release compatibility

Fresnica is still pre-release, so internal test wallet files do not require a dedicated migration path.

After public release, any persisted signer-envelope format change must include explicit versioned migration, signer identity verification, write/read-back verification, recoverable or atomic commit behavior, and previous-version fixture tests.

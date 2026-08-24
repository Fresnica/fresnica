# Software Signer Protection Model

Fresnica separates account identity, signing capability, local secret material, Core protection, client persistence, and system authentication.

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

See [Client / Core Security Contract](client-core-security.md) for the cross-client boundary and [Mobile / Rust Core Vault Contract](mobile-core-contract.md) for the mobile mapping.

## App passcode and local software signers

The default product model uses one Fresnica app passcode for ordinary local software signers.

The user sees one passcode, but each signer envelope has its own random Scrypt salt and AES-GCM nonce. Therefore the same passcode produces a different 32-byte unlock key for each protected signer.

`PasswordProtectionProvider` retains the version-1 Scrypt + AES-256-GCM envelope behavior.

The Core owns:

- signing-material payload format;
- KDF parameters and salt semantics;
- AEAD encryption/decryption;
- unlock-key derivation from passcode + envelope KDF metadata;
- secret parsing and zeroization;
- signer identity verification before an unlock key is enrolled or used;
- re-protection without exporting plaintext signing material.

Clients must not duplicate those rules.

## Standard software-signer unlock key

For a password-protected software signer, `WalletUnlockKey` is the exact 32-byte Scrypt output that encrypts that signer's canonical password envelope. The historical type name is retained because it is already part of the Core vocabulary; semantically the key is scoped to one protected software-signer envelope.

```text
App Passcode + signer salt
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

## Account identity is not signer identity

For a simple Stellar master-key wallet, the account address and signer public key are the same `G...` value. This is not a generic invariant.

Stellar accounts can authorize additional signer keys and multisig combinations, so a protected signer envelope carries a signer identity, not an account identity. Whether that signer is currently authorized for an account is determined from ledger state and client policy outside the local envelope.

A watch-only account has no local signer envelope. Adding a matching secret/mnemonic later attaches a protected signer to the existing account record; it does not create a new account.

## System authentication

Face ID, Touch ID, Android biometrics, Windows Hello, device passcode, PAM-backed local policy, or another operating-system mechanism belongs entirely to the client/platform layer.

The client decides how to protect and release the 32-byte `WalletUnlockKey`. Examples include Keychain/Keystore, a native credential vault, or another OS-specific facility. Core does not know which mechanism was used.

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

If system authentication becomes unavailable, the client can ask the user for the Fresnica app passcode, derive a fresh verified unlock key through Core, and continue using the same canonical envelope.

If the passcode or canonical envelope is re-protected with a new salt, any client-stored unlock key for the old envelope must be invalidated and re-enrolled.

## Passcode rotation / re-protection

Changing protection MUST happen inside Core:

```text
old envelope + old passcode + new passcode + expected signer
        |
Core decrypts and verifies signer
        |
Core encrypts same recovery material with fresh parameters
        |
new envelope
```

Clients MUST NOT implement password change as `Reveal -> encrypt`, because that unnecessarily crosses the declassification boundary.

When one Fresnica app passcode protects several local software signers, the client orchestrates a staged/atomic or recoverable batch of per-signer `reprotect` operations and invalidates all old system-auth unlock-key records after commit.

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

Reveal / Export is intentionally different: it accepts a fresh app passcode, not a `WalletUnlockKey`. See [Signing Material Reveal and Export](secret-export.md).

## Public release compatibility

Fresnica is still pre-release, so internal test wallet files do not require a dedicated migration path.

After public release, any persisted signer-envelope format change must include explicit versioned migration, signer identity verification, write/read-back verification, recoverable or atomic commit behavior, and previous-version fixture tests.

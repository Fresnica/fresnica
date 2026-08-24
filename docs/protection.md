# Wallet Protection Model

Fresnica separates account identity, signing, local secret material, Core protection, client persistence, and system authentication.

```text
Wallet / Account
      |
    Signer
      |
Secret Material (software signer only)
      |
Core Wallet Protection
      |
Canonical Encrypted Envelope
      |
Client Persistence
```

System authentication sits beside this flow as **client-side signer authorization**. It is not a Core protection provider and it is not a wallet encryption algorithm.

See [Client / Core Security Contract](client-core-security.md) for the cross-client boundary and [Mobile / Rust Core Vault Contract](mobile-core-contract.md) for the mobile mapping.

## App passcode and local software wallets

The default product model uses one Fresnica app passcode for ordinary software wallets.

The user sees one passcode, but each wallet envelope has its own random Scrypt salt and AES-GCM nonce. Therefore the same passcode produces a different 32-byte unlock key for each wallet.

`PasswordProtectionProvider` retains the version-1 Scrypt + AES-256-GCM envelope behavior.

The Core owns:

- signing-material payload format;
- KDF parameters and salt semantics;
- AEAD encryption/decryption;
- unlock-key derivation from passcode + envelope KDF metadata;
- secret parsing and zeroization;
- identity verification before an unlock key is enrolled or used.

Clients must not duplicate those rules.

## Standard wallet unlock key

For a password-protected wallet, the standard software-wallet unlock key is the exact 32-byte Scrypt output that already encrypts that wallet's canonical password envelope.

```text
App Passcode + wallet salt
          |
        Scrypt
          |
  WalletUnlockKey (32 bytes)
          |
     AES-256-GCM
          |
canonical wallet envelope
```

Core exposes a redacted `WalletUnlockKey` type and a verified derivation path. The verified path derives the key, opens the canonical envelope, reconstructs the signer, and checks the expected public key before returning the unlock key to a client for enrollment.

No second wallet ciphertext and no independent system wallet key are created.

## System authentication

Face ID, Touch ID, Android biometrics, Windows Hello, device passcode, PAM-backed local policy, or another operating-system mechanism belongs entirely to the client/platform layer.

The client decides how to protect and release the 32-byte `WalletUnlockKey`. Examples include Keychain/Keystore, a native credential vault, or another OS-specific facility. Core does not know which mechanism was used.

Normal software-wallet signing is therefore:

```text
Client OS authentication
        |
client releases WalletUnlockKey
        |
        v
Rust Core
same canonical envelope
  -> decrypt
  -> verify wallet identity
  -> sign
  -> drop secret-bearing signer
```

If system authentication becomes unavailable, the client can ask the user for the Fresnica app passcode, derive a fresh verified unlock key through Core, and continue using the same canonical envelope.

If the passcode or canonical envelope is re-keyed with a new salt, any client-stored unlock key for the old envelope must be invalidated and re-enrolled.

## OS boundary

Core MUST NOT implement or abstract platform authentication APIs. In particular, Core does not own:

- Keychain / Secure Enclave APIs;
- Android Keystore / StrongBox APIs;
- Windows Hello / Credential Manager / DPAPI policy;
- Linux Secret Service, PAM, desktop keyrings, or equivalent mechanisms;
- biometric prompts, app sessions, or platform authorization UX.

Different clients may implement different OS adapters. They converge only at the Core boundary: a valid `WalletUnlockKey` for normal software signing.

## Signer boundary

Hardware and external signers do not use local software-wallet unlock keys. The same client-side authorization concept may instead permit invocation of a hardware, external, remote, or future contract/passkey signer.

Normal signing should prefer the one-shot Core operation that accepts the canonical envelope plus `WalletUnlockKey`, verifies identity, signs, and drops the secret-bearing signer without returning a private key.

Reveal / Export is intentionally different: it accepts a fresh app passcode, not a `WalletUnlockKey`. See [Signing Material Reveal and Export](secret-export.md).

## Public release compatibility

Fresnica is still pre-release, so internal test wallet files do not require a dedicated migration path.

After public release, any persisted wallet-format change must include explicit versioned migration, identity verification, write/read-back verification, recoverable or atomic commit behavior, and previous-version fixture tests.

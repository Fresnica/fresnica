# Wallet Protection Model

Fresnica separates account identity, signing, local secret material, protection, persistence, and system authentication.

```text
Wallet / Account
      |
    Signer
      |
Secret Material (software signer only)
      |
Core Wallet Protection
      |
Opaque Encrypted Envelope
      |
Mobile Persistence
```

System authentication sits beside this flow as **signer authorization**. It is not itself a wallet encryption algorithm.

See [Mobile / Rust Core Vault Contract](mobile-core-contract.md) for the normative mobile integration boundary.

## App passcode and local software wallets

The default product model uses one Fresnica app passcode for ordinary software wallets.

The user sees one passcode, but each wallet envelope has its own random Scrypt salt and AES-GCM nonce. Therefore the same passcode produces different effective wallet encryption keys.

`PasswordProtectionProvider` currently retains the version-1 Scrypt + AES-256-GCM envelope behavior.

The Core owns:

- signing-material payload format;
- KDF parameters and salt semantics;
- AEAD encryption/decryption;
- secret parsing and zeroization;
- identity verification after unlock.

Mobile code must not duplicate those rules.

## System authentication

Face ID, Touch ID, Android biometrics, Windows Hello, device passcode, and similar mechanisms are authorization gates.

Biometric data is never encryption-key material.

For a software signer, successful system authentication may authorize use of platform-protected Core-compatible unlock material so the user does not need to enter the app passcode again. That shortcut must open the same canonical Core wallet envelope; it must not create a second independently encrypted copy of the wallet.

For hardware, external, remote, or future contract/passkey signers, the same authorization concept may permit signer invocation without any local wallet secret.

System-auth session policy, Keychain / Keystore integration, and biometric UI belong to the mobile or desktop platform layer.

## Current Rust implementation note

The Rust Core currently contains `SystemProtectionProvider` backed by `SystemKeyStore`. That implementation generates a separate random 256-bit wallet protection key and models `system` as a peer protection kind.

This was useful for proving the platform-storage boundary, but it is **not the target mobile product model** after the Mobile/Core contract was accepted.

Before mobile FFI is frozen, Core should decouple system authentication / signer authorization from the mutually exclusive wallet `ProtectionProvider` kind while keeping password protection as the canonical local software-wallet envelope.

## Signer boundary

Hardware and external signers do not use local secret protection. The signing abstraction remains authoritative: a signer may be backed by local protected material, secure hardware, a remote device, or a future smart-wallet/passkey mechanism.

Normal mobile signing should prefer a one-shot Core operation that unlocks protected material, verifies the expected public identity, signs, and then drops the secret-bearing signer rather than returning a private key into JavaScript.

## Public release compatibility

Fresnica is still pre-release, so internal test wallet files do not require a dedicated migration path.

After public release, any persisted wallet-format change must include explicit versioned migration, identity verification, write/read-back verification, recoverable or atomic commit behavior, and previous-version fixture tests.

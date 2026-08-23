# Wallet Protection Model

Fresnica separates account identity, signing, local secret material, and protection.

```text
Wallet / Account
      |
   Signer
      |
Secret Material (software signer only)
      |
Protection Provider
```

Password is one protection provider, not part of the wallet or signer identity.

## Password protection

`PasswordProtectionProvider` retains the existing Scrypt + AES-256-GCM behavior. Existing pre-provider password envelopes remain readable. They may be upgraded by wrapping the unchanged encrypted envelope in protection metadata; this migration does not re-encrypt the secret.

## System protection

`SystemProtectionProvider` models platform-protected wallet secrets without choosing a concrete desktop/mobile API in the Python reference.

The provider generates a random 256-bit wallet protection key, stores that key through `SystemKeyStore`, and encrypts wallet secret material with AES-256-GCM. A native implementation may map `SystemKeyStore` to macOS/iOS Keychain and Secure Enclave policy, Windows platform protection/Hello, Android Keystore, Linux Secret Service, or another suitable OS facility.

System authentication is therefore an authorization gate around access to a protection key. Biometrics are not treated as encryption keys.

## Signer boundary

Hardware and external signers do not use local secret protection at all. The signing abstraction remains authoritative: a signer may be backed by local protected material, secure hardware, a remote device, or a future smart-wallet/passkey mechanism.

## Future contract wallets

This model does not implement Stellar contract-account/passkey wallets. The intent is to avoid encoding classic-account password assumptions into Fresnica Core so a future `ContractAccount` and passkey-backed signer can be added independently.

## Python reference scope

The Python reference implements and tests the protection-provider boundary and a generic system-key-store interface. It intentionally does not bind to platform biometric APIs. Native system-authentication implementations belong in future desktop/mobile platform layers.

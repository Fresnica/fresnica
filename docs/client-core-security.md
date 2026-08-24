# Client / Rust Core Security Contract

Status: **accepted pre-release client boundary**.

This document defines the security boundary shared by Fresnica clients such as TUI/CLI, desktop, mobile, and future SDK hosts.

Client implementations may differ by operating system. Rust Core must remain platform-neutral.

## 1. Core principle

Core receives cryptographic inputs. Clients handle operating-system policy.

```text
Client / OS layer
-----------------
UI
system authentication
secure storage
session policy
persistence
        |
        | standard Core inputs
        v
Rust Core
---------
wallet envelope semantics
Scrypt / AES-GCM
unlock-key verification
identity validation
signer construction
transaction signing
secret export
```

Core MUST NOT know whether authorization came from Face ID, Touch ID, Windows Hello, Android biometrics, a desktop keyring, PAM, or another OS facility.

## 2. Two software-wallet credentials

The software-wallet boundary deliberately distinguishes two inputs.

### WalletUnlockKey: routine use

`WalletUnlockKey` is the 32-byte Scrypt output for one canonical password-protected wallet envelope.

It may be stored behind client-controlled system authentication.

Submitting a valid unlock key to Core permits routine secret-bearing operations such as constructing the software signer and signing a transaction. Core still decrypts the canonical envelope and verifies the expected wallet identity before signing.

An unlock key MUST NOT authorize secret Reveal / Export.

### App passcode: declassification and recovery

The Fresnica app passcode is the user-known recovery credential for the password-protected wallet envelope.

Submitting the passcode allows Core to derive the corresponding unlock key. It is also the required credential for explicit signing-material Reveal / Export.

Clients must request a fresh passcode for Reveal / Export; a previously released unlock key or system-authenticated session is insufficient.

## 3. System-auth enrollment

A client that wants convenient system authorization follows this flow:

```text
user enters Fresnica app passcode
        |
        v
Core derive_verified_unlock_key
  - derive Scrypt key
  - decrypt canonical envelope
  - reconstruct signer
  - verify expected public key
        |
        v
WalletUnlockKey
        |
        v
Client stores/protects key using OS-specific mechanism
```

Core does not store the key for the client and does not invoke any OS API.

The client MUST bind its stored unlock-key record to the intended wallet identity and canonical envelope/version so stale records can be invalidated safely.

## 4. Routine signing

```text
user requests transaction signing
        |
Client performs whatever local authorization policy it requires
        |
Client obtains WalletUnlockKey
        |
        v
Core sign_protected_transaction_envelope
  - decrypt same canonical envelope
  - reconstruct signer
  - verify expected public key
  - sign exact transaction
  - drop secret-bearing signer
        |
        v
signature / signed XDR
```

The client never needs the mnemonic or Stellar secret for normal signing.

## 5. Passcode fallback

If the OS authorization mechanism or secure-store record is unavailable, the wallet is still recoverable from the canonical envelope and Fresnica app passcode.

For normal signing, a client may obtain a fresh verified unlock key from Core using the passcode and use that key for the one-shot signing call.

For Reveal / Export, the client passes the fresh passcode to the dedicated export API instead.

## 6. Re-keying and invalidation

A `WalletUnlockKey` is bound to the exact password envelope because it is derived from the passcode and that envelope's Scrypt salt.

Changing the app passcode or re-encrypting the wallet with a new salt changes the unlock key.

Clients MUST invalidate any previously stored system-auth unlock key when the canonical envelope is replaced or re-keyed, then perform verified enrollment again.

## 7. Client responsibilities

Each client owns its own OS-specific implementation.

Examples:

- a macOS TUI may use Keychain and LocalAuthentication;
- a Windows CLI may use Windows Hello / platform credential facilities;
- a Linux client may use Secret Service, a desktop keyring, PAM-backed policy, or another explicit local mechanism;
- mobile may reuse Xaman-derived Keychain/Keystore and biometric infrastructure.

These adapters MUST remain outside Rust Core.

Clients are also responsible for:

- deciding when system authentication is required;
- short-lived session policy;
- storing the opaque Core envelope;
- storing/protecting the unlock key;
- deleting stale unlock-key records;
- preventing unlock keys, passcodes, mnemonics, and secrets from logs/telemetry;
- clearing temporary native buffers where practical.

## 8. Core responsibilities

Rust Core is authoritative for:

- canonical wallet envelope format;
- Scrypt parameters and derivation;
- `WalletUnlockKey` semantics;
- AES-GCM decryption;
- signing-material parsing;
- expected-public-key validation;
- one-shot protected signing;
- explicit passcode-based Reveal / Export;
- stable cryptographic error semantics.

Core MUST NOT implement a `SystemProtectionProvider`, Keychain abstraction, biometric abstraction, or OS key-store abstraction.

## 9. TUI/CLI as first native client

The first practical system-auth integration should be a native Fresnica TUI/CLI client because it can validate this boundary without mobile FFI or JavaScript concerns.

The TUI/CLI implementation should be split by platform adapters rather than pushing OS branches into Core. All adapters must produce the same Core input: a 32-byte `WalletUnlockKey` for routine software signing.

The current Python reference CLI is not the production Rust client boundary. OS-specific system-auth work should be implemented in the client that actually consumes Rust Core rather than duplicated in the Python behavioral reference.

## 10. External and future signers

The unlock-key model applies only to local software signers.

For hardware, external, remote, secure-element, or future passkey/contract signers, the client may use the same OS authorization policy to permit signer invocation, but no `WalletUnlockKey` is required when Core never owns local secret material.

The common abstraction is signer authorization; the 32-byte unlock key is specifically the software-wallet credential at the Client/Core boundary.

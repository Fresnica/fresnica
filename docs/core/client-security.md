# Client / Rust Core Security Contract

Status: **accepted pre-release client boundary**.

This document defines the security boundary shared by Fresnica clients such as TUI/CLI, desktop, mobile, and future SDK hosts.

Client implementations may differ by operating system. Rust Core must remain platform-neutral.

## 1. Core principle

Core receives cryptographic inputs. Clients handle operating-system policy and product persistence.

```text
Client / OS layer
-----------------
UI
system authentication
secure storage
session policy
account/signer persistence
        |
        | standard Core inputs
        v
Rust Core
---------
account identity parsing
protected software-signer envelope semantics
Scrypt / AES-GCM
unlock-key verification
signer identity validation
transaction signing
secret export / re-protection
external-signature verification
```

Core MUST NOT know whether authorization came from Face ID, Touch ID, Windows Hello, Android biometrics, a desktop keyring, PAM, or another OS facility.

## 2. Account identity, signer capability, and recovery source are separate

A wallet account is the chain identity the user observes. A signer is one capability that may authorize transactions for an account.

These are deliberately not the same object.

```text
AccountRecord
  identity: G... or C...
  metadata: name, network, UI state, ...

SignerRecord
  signer_public_key: G...
  kind: protected-software | hardware | external | future
  signer-specific data
```

A watch-only account is therefore an account with no locally available signer. It does not contain an empty envelope and does not require a passcode, mnemonic, secret key, or `WalletUnlockKey`.

For a simple software wallet, the account and signer public keys are normally the same `G...` value. This is only the simplest case. Stellar additional signers and multisig allow an account to be authorized by other Ed25519 public keys, so Core APIs MUST name signer identity explicitly and MUST NOT assume that the signer public key equals the transaction source account.

A `C...` contract address is an account/contract identity, not an Ed25519 signer public key. Supplying an `S...` key or mnemonic does not by itself prove ownership of a contract address. Contract/passkey authorization is a separate future capability.

Core owns Stellar address parsing and canonical identity classification. Clients may pre-check for UI purposes but MUST NOT reproduce authoritative `G...` / `C...` semantics.

A mnemonic may be a shared recovery source for several signer identities at different explicit HD indices. Recovery-source grouping is a client/application concept; it does not merge AccountRecord and SignerRecord semantics or replace any protected signer envelope. **Account != Signer != Recovery Source.**

## 3. Watch-only upgrade and downgrade

Adding signing capability to an existing watch-only classic account is an attachment operation, not creation of a second account.

```text
existing account GABC... + no local signer
        |
user supplies S... or mnemonic
        |
Core derives signer public key
        |
Core verifies expected_signer_public_key == GABC...
        |
protected software-signer envelope
```

If the derived signer does not match the expected signer identity, Core returns `identity-mismatch` and no signer is attached.

For an ordinary master-key account upgrade, `expected_signer_public_key` is the existing account `G...`. For future delegated/multisig attachment, it is the specific signer key the client intends to attach; it need not equal the account address.

Downgrading a local software wallet to watch-only is primarily a client persistence operation: after explicit user authorization, the client removes the signer reference, protected envelope, and any stored system-auth unlock key while preserving the account record and public history/cache state. No cryptographic Core operation is required to manufacture a watch-only envelope.

## 4. Protected software signer

The canonical encrypted object protects software **signing material**, not the account record itself.

The public Client/Core type is therefore conceptually:

```text
ProtectedSoftwareSigner
  signer_public_key
  envelope
```

The envelope may contain either an imported `S...` secret or mnemonic recovery material. It is opaque to clients.

Creating/importing a signer accepts an optional `expected_signer_public_key`:

- absent: normal new signer import;
- present: Core must verify the derived signer identity before returning the protected signer.

This supports watch-only upgrade without making Mobile/CLI/Swift/JNI duplicate identity checks.

For mnemonic-backed material, Core/SDK also exposes `derive_mnemonic_signer(source_envelope, passcode, expected_source_signer_public_key, index)`. It authenticates and verifies an existing mnemonic-backed protected source, derives the explicit requested index internally, and returns a new protected signer without returning the mnemonic to the client. Secret-backed sources are rejected.

## 5. Two software-signer credentials

The software-signer boundary deliberately distinguishes two inputs.

### WalletUnlockKey: routine use

`WalletUnlockKey` is the 32-byte Scrypt output for one canonical password-protected software-signer envelope.

It may be stored behind client-controlled system authentication.

Submitting a valid unlock key to Core permits routine secret-bearing operations such as reconstructing the software signer and signing a transaction. Core still decrypts the canonical envelope and verifies the expected signer public key before signing.

An unlock key MUST NOT authorize secret Reveal / Export.

### App passcode: declassification, recovery, and re-protection

The Fresnica app passcode is the user-known recovery credential for ordinary local software-signer envelopes.

Submitting the passcode allows Core to derive the corresponding unlock key. A fresh passcode is also required for explicit signing-material Reveal / Export and for changing protection.

Clients must request a fresh passcode for Reveal / Export; a previously released unlock key or system-authenticated session is insufficient.

## 6. System-auth enrollment

A client that wants convenient system authorization follows this flow:

```text
user enters Fresnica app passcode
        |
        v
Core derive_verified_unlock_key
  - derive Scrypt key
  - decrypt canonical envelope
  - reconstruct signer
  - verify expected signer public key
        |
        v
WalletUnlockKey (32 bytes)
        |
        v
Client stores/protects key using OS-specific mechanism
```

Core does not store the key for the client and does not invoke any OS API.

The client MUST treat every OS-protected unlock key as scoped to the intended signer identity and the exact canonical envelope/version that produced it. That envelope binding may live in native-record metadata or client persistence, but after an envelope is replaced the previous registration MUST be considered stale until a newly verified key is registered.

The exact OS protection topology remains client-specific. Fresnica Mobile v0.2 refines this into one device System Auth Protection Domain: initialize the auth-bound private key once, then wrap later per-signer unlock keys with the domain public key after passcode verification. That refinement does not change the Core `WalletUnlockKey` contract.

## 7. Routine signing

```text
user requests transaction signing for an account
        |
client selects an authorized local signer
        |
client performs local authorization policy
        |
client obtains signer WalletUnlockKey
        |
        v
Core sign_protected_transaction_envelope
  - decrypt signer envelope
  - reconstruct signer
  - verify expected signer public key
  - sign exact transaction
  - drop secret-bearing signer
        |
        v
signed XDR
```

The client never needs the mnemonic or Stellar secret for normal signing.

Signer selection and the question "is this signer authorized for this account now?" may depend on current ledger state and product policy. Those account-level authorization relationships are not encoded into the software-signer envelope.

## 8. Re-protection / passcode change

Changing a passcode MUST NOT be implemented as client-side `reveal -> protect`, because that unnecessarily declassifies the mnemonic or secret across the Client/Core boundary.

Core provides a re-protection operation:

```text
reprotect(
  envelope,
  current_passcode,
  new_passcode,
  expected_signer_public_key,
) -> new ProtectedSoftwareSigner
```

Core decrypts internally, reconstructs and verifies the signer identity, then encrypts the same recovery material using fresh protection parameters. Plaintext signing material is never returned to the client.

A new envelope normally has a new salt/nonce and therefore a new `WalletUnlockKey`. Clients MUST replace any OS-protected copy of the previous key with the new verified key.

Changing one global Fresnica app passcode across multiple local signers remains client orchestration: call Core re-protection for each signer, stage the resulting envelopes, verify them, then atomically commit or roll back the client-side batch. Fresnica Mobile performs its wrapped-key replacement only after that commit; its existing device-domain public key can wrap the new unlock keys without another biometric prompt.

## 9. Reveal / Export

Reveal / Export is a separate declassification operation.

```text
user explicitly requests Reveal / Export
        |
fresh Fresnica app passcode
        |
        v
Core export_signing_material
  - decrypt canonical signer envelope
  - reconstruct signer
  - verify expected signer public key
        |
        v
original mnemonic / S... material
```

A `WalletUnlockKey`, Face ID success, or an already-unlocked client session MUST NOT be sufficient to invoke this path.

## 10. External / hardware Ed25519 signers

The unlock-key model applies only to local protected software signers.

For hardware, secure-element, remote, or other external Ed25519 signers, Core should expose a transport-neutral two-step boundary rather than an FFI callback:

```text
prepare_ed25519_signing(transaction_xdr, network_passphrase)
    -> transaction_hash + public signing context

external provider signs transaction_hash

apply_ed25519_signature(
    transaction_xdr,
    network_passphrase,
    signer_public_key,
    signature,
) -> signed_xdr
```

Core recomputes the transaction hash and verifies the returned Ed25519 signature before mutating the envelope. The external provider never needs a `WalletUnlockKey` because Core never owns its private material.

## 11. Client responsibilities

Each client owns its OS-specific implementation and account/signer persistence.

Clients are responsible for:

- deciding when system authentication is required;
- maintaining account metadata and account-to-signer references;
- resolving current ledger authorization when multiple/additional signers are involved;
- short-lived session policy;
- storing opaque Core signer envelopes;
- storing/protecting unlock keys;
- deleting stale unlock-key records;
- atomic persistence when rotating a global app passcode;
- preventing unlock keys, passcodes, mnemonics, and secrets from logs/telemetry;
- clearing temporary native buffers where practical.

## 12. Core responsibilities

Rust Core is authoritative for:

- Stellar account identity parsing/classification;
- canonical software-signer envelope format;
- Scrypt parameters and derivation;
- `WalletUnlockKey` semantics;
- AES-GCM encryption/decryption;
- signing-material parsing and derivation;
- expected signer-public-key validation;
- one-shot protected signing;
- internal re-protection without client-side declassification;
- explicit passcode-based Reveal / Export;
- external-signature preparation/verification primitives;
- stable cryptographic error semantics.

Core MUST NOT implement a `SystemProtectionProvider`, Keychain abstraction, biometric abstraction, OS key-store abstraction, Realm schema, or product UI policy.

## 13. TUI/CLI as first real Core/SDK clients

The Python implementation remains the behavioral reference for product semantics, but it can also act as a real Rust Core client.

When `FRESNICA_CORE_BIN` points to the `fresnica-core` binary, or that binary is available on `PATH`, the Python TUI delegates software-signer cryptographic operations to Rust Core.

The process protocol is the first verification transport, not a requirement for every future client. The native Rust CLI consumes `fresnica-sdk` directly for account identity, wallet protection, Reveal/Export and routine passcode signing while retaining only low-level Rust Core transaction/XDR helpers where no SDK abstraction is needed. Mobile and desktop clients use the same SDK semantics through their appropriate native binding/package layer.

The CLI also exercises the watch-only signer transition directly: `attach-secret` / `attach-mnemonic` supply the existing Classic G address as `expected_signer_public_key`, and `detach-signer` removes local protected signer capability without changing the account identity.

OS-specific system-auth work still belongs to the client that releases a `WalletUnlockKey`; it is not implemented in Core or in the machine protocol.

See [`docs/core/client-protocol.md`](client-protocol.md).

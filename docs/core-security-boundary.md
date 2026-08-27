# Fresnica Core Security Boundary

## Status

This is the short normative security contract for every Fresnica product and Application Capability implementation.

Detailed implementation/reference material lives under [`core/`](core/).

## 1. Authority

**Fresnica Core** is the cryptographic/security authority.

Core/SDK owns the meaning of:

- secret and mnemonic validation/derivation;
- signer public identity derivation and verification;
- protected software-signer envelopes;
- KDF/encryption semantics;
- passcode re-protection;
- transaction hash/signature semantics;
- external Ed25519 signature verification;
- stable crypto/security error categories.

A platform may choose how to authorize access to these operations. It must not create a parallel cryptographic interpretation.

## 2. Identity model

The fundamental invariant is:

> **Account identity != Signer capability != Recovery source.**

Conceptually:

```text
AccountRecord
  chain identity / network / product metadata

SignerRecord
  signer public identity / provider metadata / opaque envelope when applicable

AccountSignerReference
  account <-> signer relationship

RecoverySource metadata
  product-owned grouping/recovery information where needed
```

A Stellar account may have multiple authorized signers. An account identity does not prove local custody of a matching private key.

## 3. Watch-only

Watch-only means:

> the account is known, but no applicable local signer capability is available.

It does **not** require a passcode, secret, mnemonic or protected signer envelope for read operations.

Attaching `S...` or mnemonic material to a classic watch-only account must derive the signer identity inside Core/SDK and verify it against the expected signer/account identity before persistence changes. A mismatch is rejected.

Detaching a local signer removes signing capability while preserving the account identity.

A `C...` contract/account identity is not an Ed25519 signer public key. An arbitrary `S...` must never be treated as direct ownership proof of a `C...` identity.

## 4. Protected software signers

Protected signer envelopes are Core-owned opaque values.

Application/platform code may persist an envelope and signer metadata but must not parse or recreate its cryptographic internals.

Plaintext secret/mnemonic material is exceptional and ephemeral. It may cross an application boundary only for deliberate operations such as:

- initial import;
- one-time generation/backup confirmation;
- explicit Reveal / Export after the required authorization.

It must not be persisted in normal application state, navigation state, Realm/SQLite truth, logs, analytics or crash reports.

## 5. Routine authorization vs higher privilege

System authentication and the Fresnica passphrase have different authority.

```text
System Auth
  -> may authorize routine signer use according to platform policy

Fresnica Passphrase
  -> protection/recovery authority
  -> required for higher-privilege operations such as Reveal/Export and passphrase rotation
```

System authentication must not silently become a replacement recovery credential.

A native `WalletUnlockKey` is routine software-signing authorization material, not a Reveal/Export credential. It must remain outside normal JavaScript/Dart/application scripting state.

Passphrase strength and credential-entry UX are Wallet/Application policy. The current Fresnica product baseline requires at least 15 Unicode scalar values when establishing new protection, while Core remains authoritative for KDF/encryption semantics rather than password-composition policy. Existing protected envelopes must remain unlockable so weak historical credentials can be rotated safely.

## 6. Transaction integrity

Review and signing must be bound to the same transaction meaning.

A transaction-producing implementation must not allow:

```text
review transaction A
        |
        v
sign/submit transaction B
```

The immutable review must correspond to the exact transaction/XDR that is subsequently authorized and signed, or the implementation must repeat review when transaction meaning changes.

Protocol challenges such as SEP-10 must preserve the same binding: a verified challenge may not be replaced with another transaction between verification and token exchange.

## 7. Signing coordination

Application Flows decide **that** signing is required.

The Ledger Authorization Capability resolves what the exact prepared transaction requires from current ledger authorization state. The Signing Coordination Capability resolves which currently available signer/provider capabilities can satisfy those requirements and coordinates collection.

Fresnica SDK/Core decides the cryptographic operation.

No individual Send/SDEX/Trustline/Dapp Flow may invent a separate signer/password/biometric cryptographic path.

## 8. Platform ownership

Platform/application code owns mechanisms such as:

- Keychain/Keystore/DPAPI/libsecret lifecycle;
- biometric/system-auth UI and session policy;
- application lock state;
- persistence and migrations;
- ledger-state acquisition used by the [Ledger Authorization Capability](capabilities/ledger-authorization.md);
- network clients and retry/cache policy;
- product UI/UX.

Those mechanisms must preserve this security contract.

## 9. External and contract signers

Hardware/external providers should use provider-neutral prepare/apply semantics where possible. Provider transport (USB/HID/BLE/vendor SDK) stays outside Core.

Passkey/smart-account authorization is a distinct signer/account model and must not be forced into protected Ed25519 signer semantics merely to reuse APIs.

## 10. Detailed references

- [Client/Core security details](core/client-security.md)
- [Signer architecture](core/signer.md)
- [Software signer protection](core/protection.md)
- [Reveal / Export](core/secret-export.md)
- [Core client protocol](core/client-protocol.md)
- [Mobile system authentication](platforms/mobile/system-auth.md)
- [Web / WASM security boundary](platforms/web/wasm-security.md)
- [External signer capability reference](capabilities/external-signer.md)
- [Passkey / smart account reference](capabilities/passkey-smart-account.md)

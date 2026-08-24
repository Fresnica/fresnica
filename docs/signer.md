# Fresnica Signer Architecture

## Principle

Account identity and signing capability are separate concepts.

An Account identifies the chain object the user is observing or operating.
A Signer identifies a cryptographic or external capability that can produce authorization material.
A valid signature proves control of the signer key; whether that signer is authorized for a particular account is a separate ledger/policy question.

```text
Wallet / client aggregate
  |
  +-- AccountRecord
  |
  +-- zero or more SignerRecords
```

A watch-only account is the zero-local-signer case. Adding or removing a local signer does not recreate the account.

## Account

Fresnica distinguishes account identity from signing key identity.

Current account kinds:

```text
Account
  |
  +-- Classic account  G...
  |      +-- Ed25519 master public key identity
  |
  +-- Contract account C...
         +-- no classic public key assumption
```

The Python reference fully implements classic-account runtime behavior. Contract-account identity is representable so future Core APIs do not encode `G... == account == signer` as a permanent assumption, but contract runtime/signing behavior is intentionally not implemented yet.

## Classic Ed25519 signer

Current classic signer interface:

```text
Signer
  |
  +-- public_key: G...
  +-- sign(transaction/signing request)
```

Current implementations include:

- software signer backed by local secret material;
- verified external Ed25519 signer.

Future implementations may include hardware-wallet or secure-element signers.

For the simplest master-key wallet:

```text
Account GABC...
Signer  GABC...
```

For Stellar additional signers / multisig:

```text
Account GABC...
  |
  +-- Signer G111...  weight N
  +-- Signer G222...  weight M
  +-- master GABC...  weight K
```

Therefore generic Core signing operations validate `expected_signer_public_key`, not an assumed account public key. Clients/network services resolve the current signer/threshold authorization for the account from ledger state.

## Protected software signer

Local mnemonic or `S...` material is represented at the Core boundary as:

```text
ProtectedSoftwareSigner
  signer_public_key: G...
  envelope: opaque encrypted recovery/signing material
```

The envelope belongs to the signer capability, not to the Account record.

An import may include `expected_signer_public_key` when attaching material to an existing account/signer slot. Core derives the signer public key and fails with `identity-mismatch` before returning a protected signer if it does not match.

This enables the ordinary watch-only master-key upgrade:

```text
Account GABC... + no local signer
        |
import matching secret/mnemonic
        |
ProtectedSoftwareSigner GABC...
        |
attach to existing account
```

It also leaves room for a future delegated signer whose `G...` differs from the account address.

## External / hardware signer

External signers do not receive a fake local password envelope or `WalletUnlockKey`.

The transport-neutral Core boundary is two-step:

```text
Core prepare_ed25519_signing
  -> exact transaction hash + public signing context

provider / hardware device
  -> 64-byte Ed25519 signature

Core apply_ed25519_signature
  -> recompute hash
  -> verify signer key/signature
  -> append decorated signature
```

This keeps transaction-hash/signature semantics authoritative in Core without requiring Rust callbacks across UniFFI/JNI/Swift boundaries.

## Contract / passkey authorization

A `C...` contract address is not an Ed25519 signer public key. Supplying a mnemonic or `S...` key cannot by itself be treated as upgrading a contract account.

Future contract/passkey signing must use an authorization model appropriate to Soroban contract authentication rather than being forced through the classic Ed25519 signer contract.

## Client/Core responsibility split

Core is authoritative for:

- signer identity derivation and validation;
- protected software-signer semantics;
- transaction hashing;
- signature generation/verification primitives;
- exact signed-XDR mutation semantics.

Clients are authoritative for:

- persisted account and signer records;
- which signer is selected for a user action;
- current ledger signer/threshold authorization;
- hardware/provider invocation;
- system-auth policy and secure storage.

The design goal is not merely to hide keys. It is to prevent account identity, signer identity, secret protection, and platform authorization from collapsing into one model again.

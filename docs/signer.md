# Fresnica Signer Architecture

## Principle

Account identity and signing capability are separate concepts.

An Account describes ownership identity.
A Signer proves ownership by producing signatures.

```
Wallet
  |
  +-- Account
  |
  +-- Signer (optional)
```

## Account

Fresnica distinguishes account identity from signing key identity.

Current account kinds:

```
Account
  |
  +-- Classic account  G...
  |      +-- Ed25519 public key
  |
  +-- Contract account C...
         +-- no classic public key assumption
```

The Python reference fully implements classic-account behavior. Contract-account identity is representable so future Core APIs do not encode `G... == account` as a permanent assumption, but contract runtime/signing behavior is intentionally not implemented yet.

A wallet may have an account without having signing capability.

## Signer

Current classic signer interface:

```
Signer
  |
  +-- public_key
  +-- sign(transaction)
```

Current implementations:

- `StellarKeypairSigner`
- `ExternalEd25519Signer`

Future classic implementations may include hardware-wallet or secure-enclave signers.

Future contract/passkey signing must not be forced through the classic Ed25519 public-key contract. It may use a separate signer/authentication implementation appropriate to Soroban contract authorization.

## Wallet Types

### Mnemonic Wallet

```
Mnemonic
   |
Keypair
   |
Classic Signer
   |
Classic Account
```

### Secret Key Wallet

```
Secret Key
   |
Keypair
   |
Classic Signer
   |
Classic Account
```

### Classic Watch-only Wallet

```
G Address / Public Key
        |
  Classic Account
        |
      Wallet
```

### Future Contract Wallet

```
C Address
   |
Contract Account
   |
contract authorization / passkey signer
```

Only the identity boundary exists in the Python reference. Soroban RPC, SAC asset behavior, SEP-45, and passkey smart-wallet signing remain future work.

## Design Goal

All transaction signing must go through an appropriate signer abstraction.

The wallet layer must not assume where keys are stored, and generic account identity must not assume every Stellar account is an Ed25519 `G...` account.

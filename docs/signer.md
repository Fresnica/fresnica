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

For Stellar:

```
Account
  = Public Identity
  = G Address
  = Public Key representation
```

A wallet may have an account without having signing capability.

## Signer

Signer interface:

```
Signer
  |
  +-- public_key()
  +-- sign(transaction)
```

Current implementation:

```
StellarKeypairSigner
        |
        +-- Stellar SDK Keypair
```

Future implementations:

- Hardware wallet signer
- Secure enclave signer
- External/remote signer
- Agent approval signer

## Wallet Types

### Mnemonic Wallet

```
Mnemonic
   |
Keypair
   |
Signer
   |
Wallet
```

### Secret Key Wallet

```
Secret Key
   |
Keypair
   |
Signer
   |
Wallet
```

### Watch-only Wallet

```
Address/Public Key
        |
     Account
        |
     Wallet
```

No signer exists. The wallet can query state but cannot sign transactions.

## Design Goal

All transaction signing must go through the Signer abstraction.

The wallet layer must not assume where keys are stored.

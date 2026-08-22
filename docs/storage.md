# Fresnica Wallet Storage Design

## Principle

Fresnica stores wallet metadata and sensitive credentials separately.

Public information can be stored in plaintext. Secrets must be encrypted.

## Public Data

Examples:

- wallet name
- account address
- public key
- network preference
- account labels
- creation time

## Sensitive Data

Must be encrypted:

- mnemonic phrase
- Stellar secret key
- private signing material

## Wallet Types

### Watch-only wallet

Stores only public information:

```
Address
Public Key
Network
Metadata
```

No signing capability.

### Self-custody wallet

Stores encrypted signing material:

```
Wallet Metadata
       +
Encrypted Secret Material
```

## Future Storage Backends

```
WalletStorage
 |
 +-- MemoryStorage
 +-- FileStorage
 +-- Mobile Keychain
 +-- Hardware-backed storage
```

The storage layer should not define signing logic. It only provides wallet state and encrypted secret material.

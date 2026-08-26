# Fresnica Runtime Architecture

## Principle

Fresnica does not replace Stellar SDK capabilities.

Stellar SDK provides:

- key management primitives
- transaction construction
- XDR
- network operations

Fresnica provides wallet lifecycle and user-facing management.

## Storage Separation

### Wallet Storage

Stores identity and control information:

```
wallets/
  main.wallet
```

Contains:

- wallet metadata
- accounts
- network preference
- encrypted signing material

Does not contain:

- balances
- transaction history
- trades
- order book data

## Data Storage

Chain data cache is independent:

```
data/
  main.sqlite
```

Contains:

- balances
- operations
- transactions
- offers
- trades

## Runtime Flow

```
fresnica balance

CLI
 |
WalletManager
 |
Wallet
 |
Account address
 |
DataStore / Stellar Adapter
 |
Display
```

Wallet answers:

"Who am I?"

DataStore answers:

"What happened on chain?"

# Fresnica Services

## Principle

Fresnica uses Stellar SDK for Stellar protocol operations. Fresnica services organize user workflows.

## Services

### BalanceService

Combines Wallet identity, StellarAdapter and DataStore.

### AvailabilityService

Calculates spendable amounts from raw account data.

Example:

```
available = balance - selling_liabilities
```

### TransactionService

Flow:

```
TransactionIntent
        |
        v
Prepare
        |
        v
Sign
        |
        v
Submit
```

Transaction construction and signing delegate to Stellar SDK and Signer.

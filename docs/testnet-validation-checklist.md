# Testnet Validation Checklist

Goal: validate the first end-to-end Fresnica wallet flow.

## Environment

```bash
cd reference/python
pip install -e ".[dev]"
```

## Wallet

```bash
fresnica wallet create testnet-wallet --network testnet
fresnica wallet list
```

Verify:

- wallet metadata exists
- secret material is encrypted
- address is a valid Stellar G address

## Funding

```bash
fresnica wallet fund
```

Verify:

- only testnet is allowed
- Friendbot response is handled

## Balance

```bash
fresnica balance --network testnet
```

Verify:

- Horizon testnet is queried
- SQLite cache is updated

## Payment

```bash
fresnica send 1 XLM to G...
```

Verify:

- review is displayed before signing
- transaction hash is stored
- result can be queried later

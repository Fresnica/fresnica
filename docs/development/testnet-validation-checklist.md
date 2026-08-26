# Testnet Validation Checklist

Goal: validate the first end-to-end Fresnica wallet flow.

## Environment

```bash
cd reference/python
uv sync --locked
uv run pytest -q
```

## Wallet

```bash
uv run fresnica --network testnet wallet create testnet-wallet
uv run fresnica wallet list
```

Verify:

- wallet metadata exists
- secret material is encrypted
- address is a valid Stellar G address

## Funding

```bash
uv run fresnica --network testnet wallet fund
```

Verify:

- only testnet is allowed
- Friendbot response is handled

## Balance

```bash
uv run fresnica --network testnet balance
```

Verify:

- Horizon testnet is queried
- SQLite cache is updated

## Payment

```bash
uv run fresnica --network testnet send 1 XLM to G...
```

Verify:

- review is displayed before signing
- transaction hash is returned
- ledger is returned when Horizon provides it

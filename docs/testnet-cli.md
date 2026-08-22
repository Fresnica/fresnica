# Fresnica Testnet CLI Flow

```bash
cd reference/python
uv sync --locked

uv run fresnica --network testnet wallet create testnet-wallet
uv run fresnica --network testnet wallet fund
uv run fresnica --network testnet balance
uv run fresnica --network testnet send 1 XLM to GDESTINATION...
```

The testnet flow validates wallet lifecycle and the transaction pipeline before
mainnet use.

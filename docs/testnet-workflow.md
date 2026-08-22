# Fresnica Testnet Workflow

```bash
cd reference/python
uv sync --locked

uv run fresnica --network testnet wallet create testnet-wallet
uv run fresnica --network testnet wallet fund
uv run fresnica --network testnet balance
uv run fresnica --network testnet send 1 XLM to GDESTINATION...
```

This flow exercises wallet creation, Friendbot funding, Horizon balance lookup,
transaction construction, signing, submission, and transaction result handling
on Stellar Testnet before mainnet use.

# Local development

## Install

```bash
cd reference/python
pip install -e ".[dev]"
```

## Network selection

Fresnica separates wallet identity from network.

```bash
fresnica wallet create --network testnet
fresnica balance --network testnet
```

For development use Testnet first. Mainnet should only be used after verifying the full transaction flow.

## Testnet flow

1. Create wallet
2. Fund account with Friendbot
3. Check balance
4. Send a small payment
5. Verify transaction hash

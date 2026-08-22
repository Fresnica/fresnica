# Local development

## Install

```bash
cd reference/python
uv sync --locked
```

The project pins Python 3.11 in `.python-version` and commits `uv.lock`. Run
commands through `uv run` so the managed `.venv` is used automatically.

## Interactive TUI

```bash
uv run fresnica
```

Keys:

```text
r  refresh balances and history
w  switch wallet
h  refresh history
s  send
q  quit
```

The TUI derives network services from the selected wallet. The send flow keeps
password entry, transaction review, confirmation, signing, submission, and
re-locking inside one interaction.

## Network selection

Fresnica separates wallet identity from network. Network is a global one-shot
CLI invocation context and must appear before the command:

```bash
uv run fresnica --network testnet wallet create testnet-wallet
uv run fresnica --network testnet balance
```

For development use Testnet first. Mainnet transaction writes should only be
used after verifying the complete flow.

## Testnet flow

```bash
uv run fresnica --network testnet wallet create testnet-wallet
uv run fresnica --network testnet wallet fund
uv run fresnica --network testnet balance
uv run fresnica --network testnet send 1 XLM to GDESTINATION...
```

## Read-only SDEX

SDEX reads use the Stellar SDK/Horizon call builders. Assets are written as
`XLM` or `CODE:GISSUER...`.

```bash
uv run fresnica --network mainnet dex orderbook XLM USDC:GISSUER...
uv run fresnica --network mainnet dex offers --limit 20
uv run fresnica --network mainnet dex trades XLM USDC:GISSUER... --limit 20
uv run fresnica --network mainnet dex candles XLM USDC:GISSUER... --resolution 1h --limit 24
```

Market cache records preserve raw Horizon JSON and also index offer direction,
trade amounts/timestamps, and OHLC/volume fields. SDEX write operations are not
part of this milestone.

Run the test suite with:

```bash
uv run pytest -q
```

# Local development

## Install

```bash
cd reference/python
uv sync --locked
```

The project pins Python 3.11 in `.python-version` and commits `uv.lock`. Run
commands through `uv run` so the managed `.venv` is used automatically.

## Local data

By default Fresnica stores local state in `~/.fresnica`. Set `FRESNICA_HOME`
to use another root directory.

```text
~/.fresnica/
  wallets/             wallet metadata and encrypted signing material
  chain-data.sqlite3   chain and SDEX cache
```

Watch-only records do not contain signing material.

## Interactive TUI

```bash
uv run fresnica
```

Keys:

```text
r  refresh balances and history
w  wallet management
h  refresh history
s  send
q  quit
```

Inside Wallet Management:

```text
S    switch to the selected wallet
A    add a watch-only wallet
Esc  close without changing wallet
```

The TUI derives network services from the selected wallet. The send flow keeps
password entry, transaction review, confirmation, signing, submission, and
re-locking inside one interaction.

## Wallet CLI

```bash
uv run fresnica wallet --help
```

Commands are grouped as lifecycle, create/import, and testnet utilities. The
canonical watch-only and Friendbot commands are:

```bash
uv run fresnica --network mainnet wallet import-watch observer G...
uv run fresnica --network testnet wallet testnet-fund
```

`watch` and `fund` remain accepted as compatibility aliases.

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
uv run fresnica --network testnet wallet testnet-fund
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

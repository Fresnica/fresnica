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

The TUI presents wallet session state explicitly:

```text
WATCH_ONLY
LOCKED
UNLOCKED
```

Main keys:

```text
w  wallet management
s  send when the wallet has signing capability
l  lock / unlock
r  refresh balances and history
h  refresh history
q  quit
```

Wallet Management supports use, add, lock/unlock, testnet funding, and delete.
Add Wallet branches into create, import-secret, import-mnemonic, and import-watch
flows. Actions are capability-aware: watch-only wallets cannot unlock, mainnet
wallets do not expose Friendbot funding, and switching wallets releases any
unlocked signing session.

Unlock is a separate workflow from Send. A write action may request unlock as a
prerequisite, but the password never appears in the payment form. An unlocked
wallet remains unlocked until explicit lock, wallet switch, or TUI exit.

Feedback is also separated by type:

```text
field validation              inline in the form
expected capability limits    notice modal
network/protocol failures      error modal (+ optional DEV diagnostics)
success/progress               main status line
```

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

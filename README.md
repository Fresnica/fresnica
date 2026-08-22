# Fresnica

Fresnica is a self-custody Stellar wallet project focused on the layer that
`stellar-sdk` intentionally does not provide: wallet lifecycle, encrypted local
storage, user workflows, caching, CLI/TUI presentation, and later reusable Core
bindings for Mobile/Desktop/SDK clients.

## Direction

1. **Python reference** — define behavior, tests, CLI/TUI workflows, and storage formats.
2. **Rust Core** — port the proven wallet/runtime behavior for production reuse.

Stellar protocol primitives stay delegated to the official SDK wherever
possible: mnemonic/key derivation, Keypair/StrKey, XDR, transaction building,
signing primitives, and Horizon access.

## Python reference

The reference implementation uses [uv](https://docs.astral.sh/uv/) for Python,
dependency, environment, and lockfile management.

```bash
cd reference/python
uv sync --locked
```

Run the interactive TUI:

```bash
uv run fresnica
```

TUI keys:

```text
r  refresh balances and recent activity
w  wallet management
h  refresh recent activity
s  send a payment
q  quit
```

Wallet Management lists local wallets in one modal. `S` switches to the selected
wallet, `A` opens the watch-only import form, and `Esc` closes without changing
the active wallet.

The TUI follows the selected wallet's stored network. Sending opens a password
prompt, then a transaction review, then submits only after confirmation. Signing
material is released again after submission or cancellation.

A network can be selected for a one-shot CLI invocation. Put the global option
before the command:

```bash
uv run fresnica --network testnet balance
```

Run tests with the locked environment:

```bash
uv run pytest -q
```

### Local data

Fresnica stores local state under `~/.fresnica` by default. Set
`FRESNICA_HOME` to move the whole data root.

```text
~/.fresnica/
  wallets/             public wallet metadata + encrypted signing material
  chain-data.sqlite3   balances, history, offers, trades, and aggregations cache
```

Watch-only wallets contain only public metadata such as name, G-address,
network, type, and timestamps. No secret or mnemonic is stored for them.

### Wallet CLI

`wallet --help` groups commands by purpose. Canonical command names are:

```text
Selection / lifecycle
  list
  use NAME
  delete NAME

Create / import
  create NAME
  import-secret NAME
  import-mnemonic NAME
  import-watch NAME G...

Testnet
  testnet-fund [--wallet NAME]
```

The older `watch` and `fund` names remain compatibility aliases.

### Testnet smoke flow

Create a disposable testnet wallet, fund it with Stellar Friendbot, inspect the
balance, then send a payment:

```bash
uv run fresnica --network testnet wallet create testnet-demo
uv run fresnica --network testnet wallet testnet-fund
uv run fresnica --network testnet balance
uv run fresnica --network testnet send 1 XLM to GDESTINATION...
```

`wallet create` displays the generated mnemonic once and stores only encrypted
signing material. `wallet testnet-fund` is rejected outside testnet. Balance,
history, and send commands also verify that the selected runtime network matches
the wallet record, preventing accidental cross-network use.

If an XLM destination does not exist, `send` reviews and submits a Stellar
`CreateAccount` operation instead of a `Payment`. Issued assets still require an
existing destination account and trustline.

### Read-only SDEX

The current SDEX milestone is intentionally read-only. Asset syntax is `XLM` or
`CODE:GISSUER...`.

```bash
# Selling XLM, buying USDC
uv run fresnica --network mainnet dex orderbook XLM USDC:GISSUER...

# Offers owned by the selected wallet
uv run fresnica --network mainnet dex offers --limit 20

# Recent trades for an explicitly oriented pair
uv run fresnica --network mainnet dex trades XLM USDC:GISSUER... --limit 20

# Horizon trade aggregations / candles
uv run fresnica --network mainnet dex candles XLM USDC:GISSUER... \
  --resolution 1h --limit 24
```

Supported aggregation resolutions are `1m`, `5m`, `15m`, `1h`, `1d`, and `1w`.
Offers, trades, and trade aggregations retain their raw Horizon JSON in the local
SQLite cache while indexing fields useful for wallet and market views.

Manage-offer and cancel-offer write operations are deliberately deferred until
the read path is stable; they will reuse the existing review/sign/submit
pipeline rather than introducing a separate signing path.

Other one-shot commands:

```bash
uv run fresnica --network mainnet wallet import-watch observer G...
uv run fresnica wallet list
uv run fresnica history
uv run fresnica send 100 XLM to G...
uv run fresnica send 25 USDC:GISSUER... to G...
```

Sensitive mnemonic/secret material is encrypted at rest. Public metadata such
as wallet name, address, and network remains readable. Chain-derived data is
kept separately in a SQLite cache. One-shot send commands release the unlocked
signer before returning.

## Current architecture

```text
CLI (Rich) / TUI (Textual)
          |
        Runtime
          |
   +------+-------+
   |              |
WalletManager   Services
   |          /    |      \
WalletStorage Balance History  DEX
               \    |      /
                 DataStore
                    |
               StellarAdapter
                    |
                stellar-sdk
```

`Wallet` represents identity plus optional signing capability. A watch-only
wallet therefore uses the same balance/history/market services but cannot sign.

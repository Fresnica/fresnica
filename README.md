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

### TUI product model

The TUI is state-driven rather than a graphical copy of CLI commands. A wallet
is always presented as one of three signing states:

```text
WATCH_ONLY   public account data only; cannot sign
LOCKED       encrypted signing material exists but is not in memory
UNLOCKED     signing material is available for the current TUI session
```

The main screen shows wallet name, network, wallet type, state, address, assets,
recent activity, and only the actions relevant to the current state. Core keys:

```text
w  wallet management
s  send (signing wallets only)
l  lock / unlock
r  refresh balances and recent activity
h  refresh recent activity
q  quit
```

`w` opens Wallet Management. From there the TUI can use, create, import, unlock,
lock, testnet-fund, or delete wallets. Add Wallet contains the same lifecycle
choices as the CLI: create mnemonic, import secret, import mnemonic, or import
watch-only.

Expected capability restrictions are shown as modal notices rather than being
written into the main status line. Form validation remains next to the field,
while network/protocol failures use an error dialog that can include `DEV`
diagnostics.

Unlocking is independent from sending. If a locked wallet starts a write action,
Fresnica opens an Unlock dialog first. Once unlocked, the wallet stays unlocked
for the TUI session until the user explicitly locks it, switches wallets, or
quits. The Send form therefore contains only payment fields; the wallet password
is not a payment parameter.

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

The older `watch` and `fund` names remain compatibility aliases. Wallet creation
uses the same `WalletManager` lifecycle model as the TUI rather than duplicating
mnemonic creation logic in the presentation layer.

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
the state-driven wallet/session UX is stable; they will reuse the same
unlock/review/sign/submit pipeline rather than introducing a separate signing
path.

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
     Wallet model/state
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

`WalletManager` owns lifecycle and session state. `Wallet` represents identity
plus optional signing capability. A watch-only wallet therefore uses the same
balance/history/market services but cannot enter a signing state.

# Fresnica Python Reference

This directory contains the original Python wallet/TUI implementation retained as a behavioral and UX reference. It is **not** the current cross-platform architecture contract. For current architecture terminology start at [`../../docs/README.md`](../../docs/README.md).

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

The TUI is state-driven rather than a graphical copy of CLI commands. Internally
a wallet session is watch-only, locked, or unlocked; the UI presents those as
capabilities instead of leaking implementation enum names.

The dashboard shows wallet identity, portfolio, liquidity positions, and recent
activity. Layout follows terminal width rather than device type:

```text
wide terminal (>= 120 columns)
+---------------- portfolio ----------------+------ recent activity ------+
| Assets                                    |                            |
| Liquidity positions                       |                            |
+-------------------------------------------+----------------------------+

narrow terminal
+---------------- portfolio ----------------+
| Assets                                    |
| Liquidity positions                       |
+-------------------------------------------+
| Recent activity                           |
+-------------------------------------------+
```

Core keys:

```text
w  wallet management
d  open Stellar DEX
s  send (signing wallets only)
l  lock / unlock
r  refresh dashboard data
h  open full History
z  show / hide zero-balance assets
q  quit
```

The footer is the canonical shortcut guide. The wallet header only shows
signing-context actions such as `S Send` and `L Unlock`; it does not duplicate
`W/D/R/H/Q`. Refresh progress appears beside dashboard data and ends with an
`Updated HH:MM:SS` timestamp.

Assets are presented as holdings rather than raw Horizon balances. XLM is always
first. Issued assets include issuer/source identity so two assets with the same
code remain distinguishable. Amounts are normalized for humans (`0E-7` -> `0`),
and the selling-liability column is labelled `In offers`. Zero-balance
trustlines are hidden by default and can be toggled with `Z`; this preference is
persisted in local settings and restored on the next TUI launch.

Liquidity-pool shares are not shown as anonymous normal assets. Fresnica resolves
the pool and displays a separate liquidity position with pool assets, share
balance, and the underlying reserve amounts represented by those shares. Pool
details are cached locally so a previously resolved position can still be
organized when the pool-detail endpoint is temporarily unavailable.

`H` opens a dedicated Activity view backed by the local operation cache. Initial
sync stores up to 200 recent operations; later refreshes request newer operations
from the latest local cursor, while `M` in Activity loads an older page. Fresnica
groups operations sharing a transaction hash into one user-facing activity while
retaining the underlying operations. Summaries are written from the current
account's point of view, for example `Sent`, `Received`, `Sell offer`,
`Added liquidity`, or `Removed trustline`.

`W` opens Wallet Management as a visible wallet list. Moving through the list
previews a wallet; `Enter` makes the highlighted wallet current, so there is no
separate Use button or shortcut. Actions are organized by meaning rather than
shown as one flat toolbar:

```text
Wallet actions   Lock / Unlock; Fund on testnet when applicable
Wallet library   Add wallet
Danger zone      Remove wallet
```

Watch-only wallets do not expose lock/unlock controls, and Friendbot funding is
hidden unless the highlighted wallet belongs to Testnet. Add Wallet contains the
same lifecycle choices as the CLI: create mnemonic, import secret, import
mnemonic, or import watch-only.

Expected capability restrictions are shown as modal notices rather than being
written into the main status line. Form validation remains next to the field,
while network/protocol failures use an error dialog that can include `DEV`
diagnostics.

Unlocking is independent from sending or trading. If a locked wallet starts a
write action, Fresnica opens an Unlock dialog first. Once unlocked, the wallet
stays unlocked for the TUI session until the user explicitly locks it, switches
wallets, or quits. Send and DEX forms therefore contain only operation fields;
the wallet password is not a payment or trading parameter.

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
  chain-data.sqlite3   balances, activity, LP metadata, offers, trades, aggregations
  settings.json        local UI preferences such as zero-balance visibility
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

CLI balance/history presentation consumes the same portfolio/activity semantics
as the TUI: issuer/source identity, human amount formatting, `Available`,
`In offers`, transaction-level activity grouping, and human-readable summaries.

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

### SDEX

Asset syntax is `XLM` or `CODE:GISSUER...`. Fresnica separates the canonical
Stellar offer stored by the ledger from the BUY/SELL view a user sees for an
explicitly oriented market pair.

For a pair `BASE/COUNTER`, both sides use one human convention:

```text
amount = BASE units
price  = COUNTER units per one BASE unit
```

For example, both `BUY 100 XRP @ 0.325 USDC/XRP` and
`SELL 100 XRP @ 0.325 USDC/XRP` use `100` and `0.325`; Fresnica chooses
`ManageBuyOffer` or `ManageSellOffer` when encoding that intent.

Read market and account state:

```bash
# Canonical order book direction
uv run fresnica --network mainnet dex orderbook XLM USDC:GISSUER...

# Current offers owned by the selected wallet
uv run fresnica --network mainnet dex offers --limit 20

# Wallet fills, aggregated by consecutive offer fills at exact price_r
uv run fresnica --network mainnet dex fills --limit 200

# Recent public trades for an explicitly oriented pair
uv run fresnica --network mainnet dex trades XLM USDC:GISSUER... --limit 20

# Horizon trade aggregations / candles
uv run fresnica --network mainnet dex candles XLM USDC:GISSUER... \
  --resolution 1h --limit 24
```

Create, update, and cancel limit offers:

```bash
# BUY and SELL both use BASE amount and COUNTER/BASE price
uv run fresnica --network mainnet dex buy  XRP:GISSUER... USDC:GISSUER... 100 0.325
uv run fresnica --network mainnet dex sell XRP:GISSUER... USDC:GISSUER... 100 0.325

# The current BUY/SELL side is inferred from the canonical offer plus this pair.
# The original operation type is neither required nor persisted as correctness state.
uv run fresnica --network mainnet dex update 12345 XRP:GISSUER... USDC:GISSUER... 90 0.33

# Cancellation uses the current canonical selling/buying/price and amount=0.
uv run fresnica --network mainnet dex cancel 12345
```

Creating an offer uses a fresh account snapshot to preflight selling balance,
existing selling liabilities, transaction fees, and the XLM minimum reserve for
the new offer subentry. If the receiving issued asset is missing a trustline,
Fresnica requires explicit approval before building `ChangeTrust + ManageOffer`
in one transaction:

```bash
uv run fresnica --network mainnet dex buy XRP:GISSUER... XLM 100 0.325 \
  --allow-trustline
```

These checks intentionally do not attempt to reimplement Stellar Core. Protocol
races, authorization/capacity rules, and update-time liability replacement remain
Core-authoritative and surface through Horizon result codes.

Current offers retain Horizon's exact `price_r`; reverse BUY views invert that
fraction rather than round-tripping a decimal reciprocal. A partially filled BUY
offer's displayed remaining BASE amount is a projection of current canonical
ledger state, not reconstruction of its historical creation intent.

Wallet fill history uses `/trades?for_account` rather than transaction history.
Raw fills are cached in SQLite. The first load establishes a recent baseline and
later refreshes continue ascending from the newest cached paging token. Fresnica
merges only consecutive fills that have the same pair, side, exact `price_r`, and
user offer ID. Trades without a user offer ID are kept separate rather than
being guessed into one order.

Supported candle resolutions are `1m`, `5m`, `15m`, `1h`, `1d`, and `1w`.
Offers, trades, account fills, and trade aggregations retain their raw Horizon
JSON in the local SQLite cache while indexing fields useful for wallet and
market views.

The TUI opens SDEX with `D` and requires an explicit `BASE/COUNTER` market before
it labels any canonical offer as BUY or SELL. The pair-scoped screen shows the
order book, current wallet offers, and wallet fill segments; reverse canonical
offers and fills are projected through the same `MarketPair` semantics used by
the CLI and Mobile/Fex. Inside the market, `B` creates BUY, `S` creates SELL,
`E` edits the selected offer without changing its projected side, `X` cancels,
and `R` refreshes. Watch-only wallets retain all read views but cannot start a
write. Locked signing wallets resume the pending DEX action after Unlock. A
missing receiving trustline gets its own explicit confirmation before the final
transaction review, and successful writes refresh both market state and the
wallet dashboard. The TUI uses the same `OpenOffer`, `OfferView`, `OfferIntent`,
`OfferService`, signer, and submit pipeline rather than duplicating protocol
logic in presentation code.

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
kept separately in a SQLite cache. One-shot write commands release the unlocked
signer before returning.

## Reference implementation architecture

The names in this diagram are Python-reference implementation names. They do not redefine the cross-platform Application Capability catalog.

```text
CLI (Rich) / TUI (Textual)
          |
 wallet / portfolio / activity / SDEX models
          |
        Runtime
          |
   +------+------------------+
   |                         |
WalletManager          Capabilities
   |              /     /      \       \
WalletStorage        Balance History   DEX   Offer
                     \       |       /       /
                         DataStore   Transactions
                              \       /
                           StellarAdapter
                                |
                            stellar-sdk
```

`WalletManager` owns lifecycle and session state. `Wallet` represents identity
plus optional signing capability. A watch-only wallet therefore uses the same
balance/history/market Capability implementations but cannot enter a signing state. Payment and
SDEX writes share the same transaction/signing/submission semantics, review
boundary, and wallet session lifecycle.

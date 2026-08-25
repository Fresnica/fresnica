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
  chain-data.sqlite3   chain, activity, LP metadata, and SDEX cache
  settings.json        local UI preferences
```

Watch-only records do not contain signing material. Operation history is cached
incrementally in SQLite. Operations that share a transaction hash are grouped
into one user-facing Activity while their individual raw operation records remain
available in the cache. Liquidity-pool details are also cached by network and
pool ID for reuse when a later pool lookup is unavailable.

## Interactive TUI

```bash
uv run fresnica
```

Main keys:

```text
w  wallet management
s  send when the wallet has signing capability
l  lock / unlock
r  refresh dashboard data
h  full Activity history
z  show / hide zero-balance assets
q  quit
```

The dashboard is responsive to terminal width. At 120 columns and wider,
portfolio data stays on the left and Recent Activity moves to the right. Narrower
terminals stack the same panes vertically. The footer is the canonical shortcut
guide; the wallet header only repeats signing-context actions.

Assets use the portfolio model rather than raw Horizon formatting: XLM sorts
first, issued assets include a shortened issuer/source, amounts remove exponent
notation and redundant trailing zeros, and selling liabilities are presented as
`In offers`. Zero-balance trustlines are hidden by default and `Z` toggles them
without a network request. That preference is persisted to `settings.json` and
restored on the next launch.

Liquidity-pool share balances are resolved into a separate Liquidity Positions
section. Fresnica loads pool reserves and total shares through the Stellar SDK,
caches the pool detail, and computes the underlying reserve amounts represented
by the wallet's shares. A previously cached pool can be used if a later pool
lookup fails, so one unavailable endpoint does not hide the rest of the wallet.

`R` refreshes the dashboard and shows `Refreshing...` / `Updated HH:MM:SS` near
the data. `H` is navigation, not another refresh shortcut: it opens an Activity
screen backed by local cache. Initial sync requests up to 200 operations; later
refreshes use the newest cached paging token to request only newer operations.
`M` inside Activity requests an older page. Transaction-level grouping is a
presentation model only; raw operation records stay intact.

Wallet Management is a list rather than a select box. Cursor movement previews a
wallet and `Enter` makes the highlighted row current, so there is no separate
Use action. Controls are grouped by intent:

```text
Wallet actions   lock / unlock; testnet funding when applicable
Wallet library   add wallet
Danger zone      remove wallet
```

Watch-only wallets hide signing controls, and Friendbot funding is not shown for
mainnet wallets. Add Wallet branches into create, import-secret, import-mnemonic,
and import-watch flows. Switching wallets releases any unlocked signing session.

Unlock is a separate workflow from Send. A write action may request unlock as a
prerequisite, but the password never appears in the payment form. An unlocked
wallet remains unlocked until explicit lock, wallet switch, or TUI exit.

Feedback is separated by type:

```text
field validation              inline in the form
expected capability limits    notice modal
network/protocol failures      error modal (+ optional DEV diagnostics)
operation success              main status line
chain refresh                  dashboard sync status
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

`watch` and `fund` remain accepted as compatibility aliases. Balance and history
use the same human-facing asset/activity semantics as the TUI, including
transaction-level Activity grouping.

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

## Native SDK Apple validation

Run the direct-consumer Apple SDK gate on a real macOS/Xcode toolchain:

```bash
bash bindings/native/scripts/validate-apple-local.sh
```

The validator now covers both iOS and macOS slices in `FresnicaSDK.xcframework` and `FresnicaSDKFFI.xcframework`. It type-checks SDK-owned Keychain/LocalAuthentication code on both platforms and verifies that independent Swift consumers can `import FresnicaSDK`.

After a React Native consumer has run `pod install`, validate the one-time Apple adapter build with:

```bash
bash adapters/react-native/apple/validate-consumer.sh /path/to/react-native-project
```

That command compiles only the canonical RN glue against the consumer's actual CocoaPods headers, verifies the adapter compatibility manifest, and checks the device/simulator adapter slices. It does not make adapter-source compilation part of normal application builds.

## GitHub main bundle artifact

A push to `main` runs the lightweight `Main bundle` workflow. It does not run the Rust, Android, Apple, or WASM validation suites. The job checks out complete history, creates a cloneable `fresnica-main.bundle`, verifies that its `main` ref and default checkout equal the pushed commit, then uploads it as the `fresnica-main-bundle` Actions artifact.

After upload, the workflow writes a `main-bundle` commit status on the pushed SHA. Its description records the immutable GitHub artifact ID and workflow run ID, while its target URL points at the artifact. This gives automated development environments a stable discovery path: read the current `main` SHA, read its `main-bundle` status, download that exact artifact ID, then verify/clone the contained bundle. No workflow-run listing or source reconstruction is required.

This artifact is the preferred GitHub-to-development handoff after a local relay push. It lets another development environment consume the exact Git commit graph instead of reconstructing source files through the GitHub contents API. Artifacts are retained for 7 days; any later `main` push creates a fresh one, and the workflow can also be started manually.

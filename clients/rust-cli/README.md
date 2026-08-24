# Fresnica Native Rust CLI

This client links `fresnica-core` directly and builds as one `fresnica` executable.
It is the second concrete Core client after the Python reference/TUI bridge.

## Current scope

The native client currently covers local wallet lifecycle and contacts, read-only
Horizon queries, reviewed payments, issued-asset trustline lifecycle, and Classic
SDEX read/write/history operations:

- `info [--wallet NAME]`
- `account [--wallet NAME] [--json]`
- `balance [--wallet NAME] [--json]` (`assets` is an alias)
- `history [--wallet NAME] [--limit N] [--json]`
- `send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]`
- `contact list`
- `contact add NAME G... [--memo TEXT]`
- `contact remove NAME`
- `trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y]`
- `trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y]`
- `trust remove CODE:GISSUER [--wallet NAME] [-y]`
- `dex orderbook SELLING BUYING [--json]`
- `dex offers [--wallet NAME] [--limit N] [--json]`
- `dex buy BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]`
- `dex sell BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]`
- `dex update OFFER_ID BASE COUNTER AMOUNT PRICE [--wallet NAME] [-y]`
- `dex cancel OFFER_ID [--wallet NAME] [-y]`
- `dex trades BASE COUNTER [--limit N] [--json]`
- `dex fills [--wallet NAME] [--limit N] [--json]`
- `dex candles BASE COUNTER [--resolution 1m|5m|15m|1h|1d|1w] [--start MS] [--end MS] [--offset MS] [--limit N] [--json]`
- `wallet list`
- `wallet use NAME`
- `wallet create NAME`
- `wallet import-secret NAME`
- `wallet import-mnemonic NAME`
- `wallet import-watch NAME G...`
- `wallet reveal [NAME]`
- `wallet backup NAME PATH`
- `wallet restore PATH [--name NAME]`
- `wallet delete NAME`

It reads and writes the same wallet record files, `.default` pointer,
`contacts.json`, and `fresnica-wallet-backup` version-1 format as the Python
reference client. The default application home is `FRESNICA_HOME` when set,
otherwise `~/.fresnica`.

Create/import/reveal cryptography is performed by the linked Rust Core. Secret,
mnemonic, BIP39-passphrase, and Fresnica-passcode prompts are read from the
controlling terminal with input hidden; they are not accepted as command-line
arguments.

Contacts are client-local public metadata. Contact names are resolved before
payment construction, an explicit `--memo` takes precedence over a contact's
default memo, and transaction review always shows the resolved G address even
when the user entered an alias.

Account state, balances, recent operations, SDEX reads, transaction preparation
and Horizon submission are client responsibilities. The CLI talks directly to
the matching public or testnet Horizon server; none of that HTTP or product
policy is moved into `fresnica-core`.

Reviewed write commands present operation-specific review and ask for
confirmation before requesting the Fresnica passcode. A short-lived verified
`WalletUnlockKey` is then derived and the exact prepared envelope is signed by
Rust Core before Horizon submission. If the HTTP submission result is uncertain,
the native client persists the locally computed transaction hash and blocks a
later same-account write until Horizon confirms it or the 210-second recovery
window expires after a not-found lookup.

Pending recovery state uses the same `pending-transactions.json` path and public
metadata schema as the Python reference. It stores only network, account,
transaction hash, kind, and submission time; signed XDR, secrets, passcodes,
unlock keys, and signer material are never persisted there. Horizon lookup
failures leave the pending record intact, and failure to persist a newly uncertain
submission produces an explicit do-not-retry warning.

The client transaction builder supports multiple operations when product
semantics require an atomic bundle. SDEX creation uses this only when the user
explicitly approves a missing receiving trustline with `--allow-trustline`, in
which case `ChangeTrust + ManageBuyOffer/ManageSellOffer` are reviewed and signed
as one transaction.

Trustline policy matches the Python reference: add reserves one additional base
reserve, the default limit is `708269837873.6765`, limit changes cannot go below
balance plus buying liabilities, and removal requires zero balance and zero
liabilities.

SDEX semantics match the Python reference and Fresnica/Fex presentation:
BASE/COUNTER is stable, price is COUNTER per BASE, BID/BUY is on the left and
ASK/SELL is on the right. Horizon BID amounts are normalized back to BASE units
using exact `price_r`. BUY uses `ManageBuyOffer` with counter as selling asset and
base as buying asset; SELL uses `ManageSellOffer` with base as selling asset.
Updates must preserve the current pair and side. Cancellation uses the ledger's
stored selling/buying orientation and does not depend on remembering the original
operation type.

Pair `trades` and `candles` are direct online Horizon projections. Wallet `fills`
use the same offer-level aggregation rule as the Python reference: only
consecutive trades with the same identified user offer, pair, side, and exact
rational price merge. Trades without a user offer ID, including non-orderbook
activity, remain separate segments. The native client deliberately does not add a
second cache implementation in this slice.

## Build

```sh
cargo build --release --manifest-path clients/rust-cli/Cargo.toml --bin fresnica
```

The executable is then:

```text
clients/rust-cli/target/release/fresnica
```

For example:

```sh
clients/rust-cli/target/release/fresnica wallet list
clients/rust-cli/target/release/fresnica account
clients/rust-cli/target/release/fresnica balance
clients/rust-cli/target/release/fresnica history --limit 20
clients/rust-cli/target/release/fresnica contact add Alice G... --memo 12345
clients/rust-cli/target/release/fresnica send 1 XLM to Alice
clients/rust-cli/target/release/fresnica trust add USDC:G...
clients/rust-cli/target/release/fresnica dex orderbook XLM USDC:G...
clients/rust-cli/target/release/fresnica dex buy XRP:G... XLM 100 0.325 --allow-trustline
clients/rust-cli/target/release/fresnica dex trades XRP:G... XLM --limit 20
clients/rust-cli/target/release/fresnica dex fills
clients/rust-cli/target/release/fresnica dex candles XRP:G... XLM --resolution 1h
```

A wallet record is bound to its configured Stellar network. Network commands
fail before contacting Horizon if the invocation network does not match the
wallet record; use `--network testnet` for a testnet wallet.

## Deliberate non-goals of this slice

Local chain-data caching, anchor protocols, and TUI presentation remain in the
Python reference for now.

The native client does not expose a raw `sign-xdr` shortcut. Routine transaction
signing stays behind client-side construction and review rather than creating a
path that bypasses product review.

OS authentication remains a client responsibility. A future platform adapter
may release a standard `WalletUnlockKey` to Core; no Keychain, biometric, PAM,
or Windows Hello logic belongs in `fresnica-core`.

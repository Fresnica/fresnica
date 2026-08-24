# Fresnica Native Rust CLI

This client links `fresnica-core` directly and builds as one `fresnica` executable.
It is the second concrete Core client after the Python reference/TUI bridge.

## Current scope

The native client currently covers local wallet lifecycle, read-only Horizon
queries, reviewed payments, issued-asset trustline lifecycle, and the first
read-only SDEX slice:

- `info [--wallet NAME]`
- `account [--wallet NAME] [--json]`
- `balance [--wallet NAME] [--json]` (`assets` is an alias)
- `history [--wallet NAME] [--limit N] [--json]`
- `send AMOUNT ASSET to G... [--wallet NAME] [--memo TEXT] [-y]`
- `trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y]`
- `trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y]`
- `trust remove CODE:GISSUER [--wallet NAME] [-y]`
- `dex orderbook SELLING BUYING [--json]`
- `dex offers [--wallet NAME] [--limit N] [--json]`
- `wallet list`
- `wallet use NAME`
- `wallet create NAME`
- `wallet import-secret NAME`
- `wallet import-mnemonic NAME`
- `wallet import-watch NAME G...`
- `wallet reveal [NAME]`
- `wallet backup NAME PATH`
- `wallet restore PATH`
- `wallet delete NAME`

It reads and writes the same wallet record files, `.default` pointer, and
`fresnica-wallet-backup` version-1 format as the Python reference client.
The default application home is `FRESNICA_HOME` when set, otherwise
`~/.fresnica`.

Create/import/reveal cryptography is performed by the linked Rust Core. Secret,
mnemonic, BIP39-passphrase, and Fresnica-passcode prompts are read from the
controlling terminal with input hidden; they are not accepted as command-line
arguments.

Account state, balances, recent operations, SDEX reads, transaction preparation
and Horizon submission are client responsibilities. The CLI talks directly to
the matching public or testnet Horizon server; none of that HTTP or product
policy is moved into `fresnica-core`.

Reviewed write commands share a small client-side transaction flow: build a
single-operation Classic envelope, present operation-specific review, ask for the
Fresnica passcode only after confirmation, derive a short-lived verified
`WalletUnlockKey`, sign through Rust Core, then submit to Horizon. Transport or
server failures during submission are reported with the locally computed
transaction hash so the user can check status before retrying.

Trustline policy matches the Python reference: add reserves one additional base
reserve, the default limit is `708269837873.6765`, limit changes cannot go below
balance plus buying liabilities, and removal requires zero balance and zero
liabilities.

SDEX order-book presentation follows the existing Fresnica/Fex market
orientation. BID/BUY is on the left and ASK/SELL is on the right. Horizon BID
amounts are normalized back to BASE units using exact `price_r`; market prices
are rendered with Stellar fixed-7-decimal semantics without turning tiny nonzero
prices into false zero.

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
clients/rust-cli/target/release/fresnica send 1 XLM to G...
clients/rust-cli/target/release/fresnica trust add USDC:G...
clients/rust-cli/target/release/fresnica dex orderbook XLM USDC:G...
clients/rust-cli/target/release/fresnica dex offers
```

A wallet record is bound to its configured Stellar network. Network commands
fail before contacting Horizon if the invocation network does not match the
wallet record; use `--network testnet` for a testnet wallet.

## Deliberate non-goals of this slice

Local chain-data caching, contacts, SDEX write operations and fill/candle
history, anchor protocols, durable pending-transaction recovery, and TUI
presentation remain in the Python reference for now.

The native client does not expose a raw `sign-xdr` shortcut. Routine transaction
signing stays behind client-side construction and review rather than creating a
path that bypasses product review.

OS authentication remains a client responsibility. A future platform adapter
may release a standard `WalletUnlockKey` to Core; no Keychain, biometric, PAM,
or Windows Hello logic belongs in `fresnica-core`.

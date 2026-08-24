# Fresnica Native Rust CLI

This client links `fresnica-core` directly and builds as one `fresnica` executable.
It is the second concrete Core client after the Python reference/TUI bridge.

## Current scope

The native client currently covers local wallet lifecycle, read-only Horizon
queries, and the first reviewed transaction write path:

- `info [--wallet NAME]`
- `account [--wallet NAME] [--json]`
- `balance [--wallet NAME] [--json]` (`assets` is an alias)
- `history [--wallet NAME] [--limit N] [--json]`
- `send AMOUNT ASSET to G... [--wallet NAME] [--memo TEXT] [-y]`
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

Account state, balances, recent operations, transaction preparation and Horizon
submission are client responsibilities. The CLI talks directly to the matching
public or testnet Horizon server; none of that HTTP or product policy is moved
into `fresnica-core`.

For `send`, the client fetches current source account state and ledger fee/reserve
parameters, checks available balance, constructs a Classic transaction, and
shows the complete payment review before asking for the Fresnica passcode. Only
after confirmation does it derive a short-lived verified `WalletUnlockKey` and
invoke the existing Core protected-signing path. Transport or server failures
during submission are reported with the locally computed transaction hash so the
user can check status before retrying.

## Build

```sh
cargo build --release --manifest-path clients/rust-cli/Cargo.toml --bin fresnica
```

The executable is then:

```text
clients/rust-cli/target/release/fresnica
```

For example, it can inspect the same local wallet library used by the Python
client, query its network account, and send after review:

```sh
clients/rust-cli/target/release/fresnica wallet list
clients/rust-cli/target/release/fresnica info
clients/rust-cli/target/release/fresnica account
clients/rust-cli/target/release/fresnica balance
clients/rust-cli/target/release/fresnica history --limit 20
clients/rust-cli/target/release/fresnica send 1 XLM to G...
```

A wallet record is bound to its configured Stellar network. Network commands
fail before contacting Horizon if the invocation network does not match the
wallet record; use `--network testnet` for a testnet wallet.

## Deliberate non-goals of this slice

Local chain-data caching, contacts, SDEX, anchor protocols, durable pending-
transaction recovery, and TUI presentation remain in the Python reference for
now.

The native client does not expose a raw `sign-xdr` shortcut. Routine transaction
signing stays behind client-side construction and review rather than creating a
path that bypasses product review.

OS authentication remains a client responsibility. A future platform adapter
may release a standard `WalletUnlockKey` to Core; no Keychain, biometric, PAM,
or Windows Hello logic belongs in `fresnica-core`.

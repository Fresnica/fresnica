# Fresnica Native Rust CLI

This client links `fresnica-core` directly and builds as one `fresnica` executable.
It is the second concrete Core client after the Python reference/TUI bridge.

## Current scope

The first native slice intentionally covers local wallet lifecycle only:

- `info [--wallet NAME]`
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

## Build

```sh
cargo build --release --manifest-path clients/rust-cli/Cargo.toml --bin fresnica
```

The executable is then:

```text
clients/rust-cli/target/release/fresnica
```

For example, it can inspect the same local wallet library used by the Python
client:

```sh
clients/rust-cli/target/release/fresnica wallet list
clients/rust-cli/target/release/fresnica info
```

## Deliberate non-goals of this slice

Network account state, Horizon history, assets, contacts, SDEX, anchor
protocols, transaction construction, submission, and TUI presentation remain in
the Python reference for now.

The native client also does not expose a raw `sign-xdr` shortcut. Routine
transaction signing should be added together with the client-side transaction
review semantics rather than creating a path that bypasses product review.

OS authentication remains a client responsibility. A future platform adapter
may release a standard `WalletUnlockKey` to Core; no Keychain, biometric, PAM,
or Windows Hello logic belongs in `fresnica-core`.

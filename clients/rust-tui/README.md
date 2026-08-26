# Fresnica Rust TUI

`fresnica-tui` is the native engineering/reference terminal UI for Fresnica.
It consumes `clients/rust-client`; it does not own separate wallet, crypto, or
Horizon semantics.

Current first slice:

- selected wallet identity and signer capability;
- network-scoped wallet switching for the current session;
- Horizon balances/liabilities;
- recent account activity;
- manual refresh.

Run after building with Rust:

```bash
cargo run --manifest-path clients/rust-tui/Cargo.toml -- --network testnet
```

Use `--home PATH` or `FRESNICA_HOME` to point at an isolated wallet directory.

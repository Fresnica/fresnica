# Fresnica Rust TUI

`fresnica-tui` is the native engineering/reference terminal UI for Fresnica.
It consumes `reference/rust-client`; it does not own separate wallet, crypto, or
Horizon semantics.

Current reference slice:

- selected wallet identity and signer capability;
- network-scoped wallet switching for the current session;
- Horizon balances/liabilities;
- recent account activity;
- manual refresh;
- reviewed XLM/issued-asset payment preparation through `fresnica-client`;
- masked passcode entry and SDK-backed submission with shared pending-transaction protection.

The TUI owns interaction state and confirmation. Payment validation, exact review data, transaction construction, signing handoff, submission, and pending retry protection are shared client-service behavior rather than copies of CLI command handlers.

Run after building with Rust:

```bash
cargo run --manifest-path clients/rust-tui/Cargo.toml -- --network testnet
```

Use `--home PATH` or `FRESNICA_HOME` to point at an isolated wallet directory.

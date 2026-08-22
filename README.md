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

```bash
cd reference/python
python -m pip install -e ".[dev]"
```

Run the interactive TUI:

```bash
fresnica
```

Or use one-shot command mode:

```bash
fresnica wallet create main
fresnica wallet watch observer G...
fresnica wallet list
fresnica balance
fresnica history
fresnica send 100 XLM to G...
fresnica send 25 USDC:GISSUER... to G...
```

Sensitive mnemonic/secret material is encrypted at rest. Public metadata such
as wallet name, address, and network remains readable. Chain-derived data is
kept separately in a SQLite cache.

## Current architecture

```text
CLI (Rich) / TUI (Textual)
          |
        Runtime
          |
   +------+-------+
   |              |
WalletManager   Services
   |              |
WalletStorage  DataStore
                  |
             StellarAdapter
                  |
              stellar-sdk
```

`Wallet` represents identity plus optional signing capability. A watch-only
wallet therefore uses the same balance/history services but cannot sign.

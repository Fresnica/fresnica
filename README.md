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

Run the interactive TUI on the default mainnet context:

```bash
uv run fresnica
```

A network can be selected for the whole invocation. Put the global option
before the command:

```bash
uv run fresnica --network testnet balance
```

Run tests with the locked environment:

```bash
uv run pytest -q
```

### Testnet smoke flow

Create a disposable testnet wallet, fund it with Stellar Friendbot, inspect the
balance, then send a payment:

```bash
uv run fresnica --network testnet wallet create testnet-demo
uv run fresnica --network testnet wallet fund
uv run fresnica --network testnet balance
uv run fresnica --network testnet send 1 XLM to GDESTINATION...
```

`wallet create` displays the generated mnemonic once and stores only encrypted
signing material. `wallet fund` is rejected outside testnet. Balance, history,
and send commands also verify that the selected runtime network matches the
wallet record, preventing accidental cross-network use.

Other one-shot commands:

```bash
uv run fresnica wallet watch observer G...
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

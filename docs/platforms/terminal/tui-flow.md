# Fresnica CLI and TUI Flow

## Modes

Fresnica supports two entry styles.

One-shot command:

```
fresnica send 100 XLM to G...
```

Interactive mode:

```
fresnica
```

opens the TUI.

## One-shot command

The command executes a complete workflow:

```
Command
  -> WalletManager
  -> Wallet state check
  -> Availability check
  -> Transaction build
  -> Review
  -> Sign
  -> Submit
```

## Interactive TUI

The TUI provides:

- wallet overview
- balances and liquidity positions
- transaction-level activity
- send flow
- local contact aliases
- wallet management
- pair-scoped SDEX trading

CLI and TUI use the same wallet/runtime Capability implementations and review semantics. Send
destinations are resolved through the same local contact resolver before the
Payment/Transaction Capability implementation is called. An explicit memo overrides a contact's default memo,
and the final review shows both the contact name and the resolved Stellar
address.

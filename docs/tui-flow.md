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
- balances
- transactions
- send flow
- account management
- future SDEX terminal

The same services are used by CLI and TUI.

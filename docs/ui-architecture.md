# Fresnica UI Architecture

## Interfaces

Fresnica provides multiple user interfaces over the same service layer.

```
                 Services
                    |
        +-----------+-----------+
        |                       |
       CLI                     TUI
        |                       |
      Rich                 Textual
```

## CLI mode

Example:

```
fresnica send 100 XLM to G...
```

CLI is designed for:

- scripts
- automation
- agents
- quick actions

## Interactive mode

Example:

```
fresnica
```

Starts TUI mode.

TUI provides:

- wallet overview
- balances
- history
- send flow
- SDEX terminal

## Rich

Rich is the rendering layer for command output:

- tables
- panels
- confirmations
- progress

## Textual

Textual is the application framework for interactive terminal UI.

Both interfaces consume the same Fresnica services.

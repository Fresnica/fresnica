# Fresnica Application Flow

## Entry Points

### Interactive mode

```
fresnica
    |
    v
 TUI
```

### Command mode

```
fresnica send ...
    |
    v
 CLI command
```

Both share the same Runtime.

```
Runtime
 |
 +-- WalletManager
 +-- Services
 +-- StellarAdapter
 +-- DataStore
```

No interface owns business logic.

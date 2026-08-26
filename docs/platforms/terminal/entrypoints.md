# Fresnica Terminal Entrypoints

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
 +-- Application Capabilities
 +-- StellarAdapter
 +-- DataStore
```

No terminal interface owns shared wallet semantics. CLI and TUI implement terminal-specific Application Flows over the same Capability layer.

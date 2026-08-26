# Terminal Engineering Clients

The Rust CLI and TUI are engineering/reference products that consume the Rust Application Capability implementation in `clients/rust-client`.

They are useful executable references, but their command syntax, local file storage and terminal interaction are not cross-platform contracts.

- [CLI/TUI entrypoints](entrypoints.md)
- [CLI send flow](cli-send-flow.md)
- [TUI flow](tui-flow.md)
- [Terminal system authorization](system-auth.md)
- [Terminal UI architecture](ui-architecture.md)
- [Runtime](runtime.md)
- [Wallet storage](storage.md)
- [History cache](history-cache.md)

When a terminal behavior proves to be stable wallet semantics, promote it into the relevant [`../../capabilities/`](../../capabilities/README.md) contract instead of asking other platforms to copy terminal implementation structure.

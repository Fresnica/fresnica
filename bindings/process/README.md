# Fresnica Process Binding

`fresnica-process-binding` is the optional versioned stdin/stdout adapter over `fresnica-sdk` for RefPython, conformance tools and suitable non-Rust Desktop hosts. Rust applications link the SDK directly; native applications normally use the Native SDK.

The host owns persistence, networking, OS authentication, lifecycle and UI. The binding owns JSON/base64/process transport only; SDK/Core remain authoritative for identity, protection, signing and errors. Sensitive values travel over stdin, never argv/environment variables.

`PROCESS_BINDING_API_VERSION` is independent from SDK/Core API versions. Build with `cargo build --release --manifest-path bindings/process/Cargo.toml --bin fresnica-process`.

## Security classification

This binary is a **privileged owner/host binding**, not a sandbox or authorization boundary. API v2 retains the owner-only operations that transport passphrases, generated/revealed mnemonic or secret material, and raw `WalletUnlockKey` values over stdin/stdout, and adds the SDK v4 Soroban authorization-entry protected/passcode and external Ed25519 prepare/apply operations for the concrete RefPython trusted-host consumer. The parent process must already be trusted to receive those values and must control child-process integrity, pipes, logging, crash collection and lifecycle.

Do not expose `fresnica-process` directly as a network daemon, MCP/agent tool, renderer/browser API, untrusted plugin interface or shared multi-tenant service. A future Agent Access API must be a separate, narrower surface that cannot Reveal material, derive/export unlock keys or accept owner passphrases.

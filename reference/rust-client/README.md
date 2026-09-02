# Fresnica Rust Capability Reference

`fresnica-client` is the reusable Rust reference implementation for Fresnica Application Capabilities.

It exists for three reasons:

1. reusable Rust application behavior for Rust consumers;
2. executable evidence for shared Capability semantics;
3. a source of regression/conformance cases that can be promoted into shared contracts.

It is **not** the Fresnica Core, not a product UI contract, and not mandatory runtime code for Mobile/Web/Desktop products.

## Dependency boundary

```text
Application Flow / product
        |
        v
fresnica-client
        |
        +--> fresnica-sdk / Fresnica Core
        +--> Stellar Horizon / RPC adapters
        +--> reference repositories/storage
```

Direct `fresnica-core` use is intentionally limited to reviewed low-level gaps while routine identity, protection, Reveal/Export and signing semantics go through `fresnica-sdk`. Repository CI enforces that boundary with `scripts/validate-rust-sdk-boundary.sh`.

## Product use

The current Rust CLI/TUI still consume this crate while they are being extracted into the independent `fresnica-terminal` product repository. Future terminal releases may pin a specific Fresnica shared-repository revision rather than requiring source co-location.

Platform products may instead implement the same Application Capability contracts with their native Stellar SDK and persistence stack.

## Validation

```bash
cargo test --manifest-path reference/rust-client/Cargo.toml
```

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
        +--> official Stellar protocol/client crates
        +--> Stellar Horizon / RPC adapters
        +--> reference repositories/storage
```

Direct `fresnica-core` use is intentionally limited to reviewed low-level gaps while routine identity, protection, Reveal/Export and signing semantics go through `fresnica-sdk`. Repository CI enforces that boundary with `scripts/validate-rust-sdk-boundary.sh`.

Network runtime configuration belongs at this capability/client boundary. `NetworkProfile::for_network(...)` supplies the shared defaults; products may override Horizon and, when a capability requires it, Stellar RPC before constructing `FresnicaClient`. The selected Stellar network remains the cryptographic identity, while provider URLs are replaceable runtime services. Testnet has a known RPC default; mainnet contract invocation requires an explicit RPC endpoint until Fresnica intentionally adopts a stable default. Core and the stateless `fresnica-sdk` do not own provider URLs.

Contract invocation follows the same boundary. `FresnicaClient::contract_interface(...)` resolves the deployed Contract Spec through the official Stellar RPC/XDR/spec crates and exposes provider-neutral function/parameter metadata. Rust products pass function names plus named `ContractArgumentInput` text values. `fresnica-client` delegates Soroban value parsing and normalized JSON conversion to the official `soroban-spec-tools` implementation instead of maintaining a parallel ABI parser, then owns Fresnica-specific simulation review, authorization, envelope signing, pending-transaction safety and RPC submission. Product code does not construct XDR, interpret `ScSpecEntry`, or instantiate an RPC gateway.

Remote Contract Spec discovery remains a thin application adapter because Stellar currently exposes that orchestration from the full `soroban-cli` product crate rather than a smaller reusable client crate. Fresnica composes the lower-level official `stellar-rpc-client`, `soroban-spec`, `stellar-asset-spec` and XDR APIs for Stellar Asset Contract, Wasm and CAP-85 external-reference executables without importing Stellar CLI's identity/configuration/signing product model.

## Product use

The current Rust CLI/TUI still consume this crate while they are being extracted into the independent `fresnica-terminal` product repository. Future terminal releases may pin a specific Fresnica shared-repository revision rather than requiring source co-location.

Platform products may instead implement the same Application Capability contracts with their native Stellar SDK and persistence stack.

## Validation

```bash
cargo test --manifest-path reference/rust-client/Cargo.toml
```

# Fresnica Core (Rust)

This directory is the production Rust Core for Fresnica.

The Python reference remains the behavioral authority while stable semantics are ported. Rust code should reuse established Stellar primitives and reproduce existing cross-language test-vector behavior instead of introducing a parallel wallet model.

## Initial scope

The first slice contains account identity only:

- Classic Stellar accounts (`G...`)
- Contract account identities (`C...`)
- explicit separation between account address and classic Ed25519 public key

Contract runtime behavior is not implemented here yet. Soroban RPC, SAC balances, SEP-45, smart-wallet/passkey authorization, transaction building, signers, storage, SDEX, anchors, and UI remain outside this first slice.

## Validation

```sh
cargo test --manifest-path core/rust/Cargo.toml
```

Future slices should consume `spec/test-vectors` where a stable language-neutral contract already exists.

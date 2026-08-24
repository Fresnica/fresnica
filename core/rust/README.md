# Fresnica Core (Rust)

This directory is the production Rust Core for Fresnica.

The Python reference remains the behavioral authority while stable semantics are ported. Rust code should reuse established Stellar primitives and reproduce existing cross-language test-vector behavior instead of introducing a parallel wallet model.

## Current scope

Implemented production primitives currently include:

- Classic Stellar account identities (`G...`)
- Contract account identities (`C...`) without contract runtime assumptions
- SEP-0005 deterministic Classic public-key derivation
- Classic Ed25519 software signer
- Classic transaction envelope hashing and decorated-signature attachment through the official `stellar-xdr` crate

Transaction building, network submission, storage, SDEX, anchors, Soroban account authorization, passkeys, and UI remain outside the current Rust Core slice.

## Signing boundary

Classic transaction signing deliberately accepts an exact 32-byte Stellar transaction hash. It is not an arbitrary-message signing API.

Arbitrary message signing is reserved as a separate future capability following **SEP-53 (Sign and Verify Messages)**. That extension must preserve SEP-53 domain separation (`Stellar Signed Message:\n`) rather than widening the transaction-signing method to accept arbitrary bytes.

## Validation

```sh
cargo test --manifest-path core/rust/Cargo.toml
```

Future slices should consume `spec/test-vectors` where a stable language-neutral contract already exists.

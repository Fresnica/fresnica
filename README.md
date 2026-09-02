# Fresnica

Fresnica is a self-custody Stellar wallet/security foundation with shared cross-platform wallet semantics and a Rust cryptographic/security Core.

This repository is the shared **Core / SDK / Specification / Reference** repository. Product UI/application repositories are expected to evolve independently while feeding stable semantics back into these contracts.

## Architecture

```text
Application Flows        product intent / UI / UX
        |
        v
Application Capabilities shared wallet semantics
        |
        +--> Fresnica SDK / Rust Core   cryptographic authority
        +--> Stellar/network gateways   chain/protocol mechanisms
        +--> repositories/platform ports
```

The Rust `fresnica-client` under `reference/rust-client` is a reusable/reference Capability implementation. RefPython remains the readable semantic laboratory. Products may reuse those implementations or implement the same contracts independently.

> **Unify semantics, not source code. Unify contracts, not UI.**

Start with [`docs/README.md`](docs/README.md) for the canonical documentation map and vocabulary.

## Repository map

```text
core/rust/               Rust cryptographic/protocol Core
sdk/                     platform-neutral SDK contracts and compatibility tooling
bindings/                Native, Process and WASM delivery bindings
adapters/                framework adapter source/tooling
providers/               provider/reference integrations
reference/python/        readable behavior/UX/protocol reference laboratory
reference/rust-client/   Rust Application Capability reference implementation
spec/test-vectors/       cross-language semantic/crypto fixtures
docs/                    architecture, capability, platform and development docs
scripts/                 repository-level validation helpers
```

The native terminal products now live in the independent [`Fresnica/fresnica-terminal`](https://github.com/Fresnica/fresnica-terminal) repository and consume the shared Rust capability reference from here.

## Product boundaries

- **Fresnica Core / SDK** owns cryptographic meaning, signer identity validation, protected software signers and signing/verification semantics.
- **Application Capabilities** define reusable wallet meaning such as Payment, Transaction, Trustline, SDEX and Anchor.
- **Application Flows** own product sequence, confirmation and UI/UX.
- **Reference implementations** prove semantics; they are not mandatory runtime dependencies for every product.
- **Products** choose platform-appropriate Stellar SDKs, persistence, network clients and UI frameworks while preserving normative contracts.

Current product repositories include `fresnica-mobile` and [`fresnica-terminal`](https://github.com/Fresnica/fresnica-terminal); Web/Desktop products should follow the same boundary rather than being folded back into this repository.

## Hardware signer status

The provider-neutral External Signer boundary is proven end-to-end with a physical Ledger on macOS/Testnet using Stellar app 6.0.3 and Classic clear signing with Blind Signing disabled. Hardware transport remains above Core; no Ledger-specific private-key model was added to Core.

See [`docs/capabilities/external-signer.md`](docs/capabilities/external-signer.md).

## Development

Operational setup and validation instructions live under [`docs/development/`](docs/development/README.md).

Reference implementations:

- [`reference/python/README.md`](reference/python/README.md)
- [`reference/rust-client/README.md`](reference/rust-client/README.md)

Native SDK/platform integration starts at:

- [`docs/sdk/README.md`](docs/sdk/README.md)
- [`docs/platforms/README.md`](docs/platforms/README.md)

## Project state

- [`docs/roadmap.md`](docs/roadmap.md) - project phases and direction.
- [`docs/tasks.md`](docs/tasks.md) - implementation checklist.
- [`docs/handoff.md`](docs/handoff.md) - compact continuation state.

These state documents may change faster than the common contracts linked from [`docs/README.md`](docs/README.md).

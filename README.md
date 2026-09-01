# Fresnica

Fresnica is a self-custody Stellar wallet project with shared cross-platform wallet semantics and a Rust cryptographic/security Core.

The repository contains Core/SDK infrastructure, reusable Rust Application Capability implementations, engineering CLI/TUI clients, platform integration contracts and the retained Python reference wallet.

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

The Rust `fresnica-client` is the reusable/reference Capability implementation for Rust CLI/TUI. Mobile, Web and Desktop may implement the same Capability contracts with platform-appropriate Stellar SDKs, storage and infrastructure.

> **Unify semantics, not source code. Unify contracts, not UI.**

Start with [`docs/README.md`](docs/README.md) for the canonical documentation map and vocabulary.

## Repository map

```text
core/rust/              Rust cryptographic/protocol Core
sdk/                    platform-neutral SDK contracts and compatibility tooling
bindings/               Native, Process and WASM bindings
adapters/               framework adapter source/tooling
clients/rust-client/    Rust Application Capability reference implementation
clients/rust-cli/       Rust CLI engineering client
clients/rust-tui/       Rust TUI engineering client
providers/              external/provider integrations
reference/python/       retained Python behavior/UX reference
spec/test-vectors/      cross-language semantic/crypto fixtures
docs/                   architecture, capability, platform and development docs
```

## Current product boundaries

- **Fresnica Core / SDK** owns cryptographic meaning, signer identity validation, protected software signers and transaction signing/verification.
- **Application Capabilities** define reusable wallet semantics such as Payment, Transaction, Trustline, SDEX and Anchor.
- **Application Flows** own product sequence, confirmation and UI/UX.
- Platform implementations may choose their own Stellar SDK, persistence, network client and UI framework while preserving normative contracts.

The independent `fresnica-mobile` product is expected to organize its UI/application code with Mobile Features that implement Application Flows and consume Mobile Capability implementations.

## Development

Operational setup and validation instructions live under [`docs/development/`](docs/development/README.md).

The retained Python reference has its own guide at [`reference/python/README.md`](reference/python/README.md).

Native SDK/platform integration starts at:

- [`docs/sdk/README.md`](docs/sdk/README.md)
- [`docs/platforms/README.md`](docs/platforms/README.md)

## Project state

- [`docs/roadmap.md`](docs/roadmap.md) - project phases and direction.
- [`docs/tasks.md`](docs/tasks.md) - implementation checklist.
- [`docs/handoff.md`](docs/handoff.md) - compact continuation state.

These state documents may change faster than the five common contracts linked from [`docs/README.md`](docs/README.md).

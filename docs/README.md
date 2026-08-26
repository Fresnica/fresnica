# Fresnica Documentation

This directory is organized around shared contracts first and implementation detail second.

## Start here

All Fresnica products should begin with these five documents, in order:

1. [Architecture](architecture.md) - the shared layering and canonical vocabulary.
2. [Application Flows](application-flows.md) - product intent, sequencing and UI/UX ownership.
3. [Application Capabilities](application-capabilities.md) - cross-platform wallet semantics.
4. [Core Security Boundary](core-security-boundary.md) - security invariants that implementations must not redefine.
5. [Platform Implementation](platform-implementation.md) - how each runtime may implement the contracts independently.

The compact model is:

```text
Application Flow
  user goal / product sequence / UI state
        |
        v
Application Capabilities
  shared wallet semantics
        |
        +----------------------+----------------------+------------------+
        |                      |                      |                  |
        v                      v                      v                  v
Fresnica SDK / Core       Stellar SDK/Gateway   Repositories       Platform ports
crypto/security authority chain mechanisms       durable state      OS/runtime mechanisms
```

> **Flows are product-specific. Capabilities are semantically shared. Fresnica Core is authoritative for cryptographic meaning.**

## Common contracts

- [Architecture](architecture.md)
- [Application Flows](application-flows.md)
- [Application Capabilities](application-capabilities.md)
- [Core Security Boundary](core-security-boundary.md)
- [Platform Implementation](platform-implementation.md)

## Capability references

Detailed notes that support individual capabilities live in [`capabilities/`](capabilities/):

- [Anchor](capabilities/anchor.md)
- [External / hardware signer](capabilities/external-signer.md)
- [Network configuration](capabilities/network.md)
- [Passkey / smart account](capabilities/passkey-smart-account.md)

These references may contain implementation status and provider-specific detail. The canonical capability names and maturity levels remain in [Application Capabilities](application-capabilities.md).

## Core references

Detailed Core/security contracts live in [`core/`](core/):

- [Core client protocol](core/client-protocol.md)
- [Client/Core security details](core/client-security.md)
- [Signer architecture](core/signer.md)
- [Software signer protection](core/protection.md)
- [Reveal / export](core/secret-export.md)

The short cross-platform security authority is [Core Security Boundary](core-security-boundary.md).

## Platform references

Platform documents describe implementation choices, packaging and UX integration. They do not redefine shared Capability semantics.

### Mobile

Start with the five common contracts above, then read:

- [Mobile / Native SDK bindings](platforms/mobile/bindings.md)
- [Mobile SDK usage](platforms/mobile/sdk-usage.md)
- [Framework adapter](platforms/mobile/framework-adapter.md)
- [System authentication](platforms/mobile/system-auth.md)
- [Security vault mapping](platforms/mobile/security-vault-contract.md)
- [Application migration reference](platforms/mobile/app-migration-pr81-pr84.md)
- [React Native upgrade playbook](platforms/mobile/react-native-upgrade-playbook.md)

The independent `fresnica-mobile` project's Feature-first architecture is a Mobile implementation of **Application Flows**. A Mobile `Feature` may implement one or more Flows and consumes Application Capabilities through Mobile-owned implementations/ports.

### Desktop

- [Desktop SDK contract](platforms/desktop/sdk-contract.md)

### Web

- [Web / WASM security boundary](platforms/web/wasm-security.md)

### Terminal engineering clients

- [CLI/TUI entrypoints](platforms/terminal/entrypoints.md)
- [CLI send flow](platforms/terminal/cli-send-flow.md)
- [TUI flow](platforms/terminal/tui-flow.md)
- [Terminal system authorization](platforms/terminal/system-auth.md)
- [Terminal UI architecture](platforms/terminal/ui-architecture.md)
- [Terminal runtime](platforms/terminal/runtime.md)
- [Terminal wallet storage](platforms/terminal/storage.md)
- [Terminal history cache](platforms/terminal/history-cache.md)

## SDK and development

- [Native SDK release contract](sdk/native-release.md)
- [Local development](development/local-development.md)
- [Testnet CLI](development/testnet-cli.md)
- [Testnet SDEX](development/testnet-sdex.md)
- [Testnet validation checklist](development/testnet-validation-checklist.md)
- [Testnet workflow](development/testnet-workflow.md)

## Project state and decisions

- [Roadmap](roadmap.md)
- [Tasks](tasks.md)
- [Current handoff](handoff.md)
- [Architecture decision log](decisions/architecture.md)

`roadmap.md`, `tasks.md` and `handoff.md` describe current project state. They are not permanent architecture contracts and may age faster than the five common documents.

## Legacy terminology

Historical documents/code may still use `Service` for what is now called an **Application Capability**, or use Mobile `Core` for an application capability layer. Do not copy those names into new cross-project architecture.

The legacy terminology note is retained at [`archive/services.md`](archive/services.md).

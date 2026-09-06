# Architecture Decisions

Decision records preserve historical reasoning and compatibility context.

- [Shared Application Client Boundary](shared-application-client.md) — current first-party native application-runtime target
- [Architecture decision log](architecture.md) — historical and domain-specific decisions
- [Protocol-demand development](protocol-demand-development.md)

The current common contracts in the parent directory remain authoritative for semantic behavior. When older platform-implementation guidance conflicts with the newer Shared Application Client decision, the newer decision governs first-party native runtime ownership while preserving the existing SDK/Core security boundaries.

# Fresnica Roadmap

Updated: 2026-09-01

Fresnica is a shared Stellar wallet/security foundation. The repository is a monorepo for development convenience, but its components evolve at different speeds and should be treated as logically separate projects.

## Direction

```text
Stellar protocol / CAP / stable standards
                    |
                    v
              Fresnica Core
        slow security/protocol authority
                    |
                    v
               Fresnica SDK
        stable cross-language semantics
                    |
       +------------+-------------+
       |                          |
       v                          v
   RefPython                  Rust Client
semantic laboratory       wallet capability reference
       |                          |
       +------------+-------------+
                    |
       Mobile / Web / Desktop / Agent
             independent products
```

The target baseline is [`development/modern-stellar-core-capability-baseline.md`](development/modern-stellar-core-capability-baseline.md).

## Modern Stellar foundation

### M0 — Core protocol/security baseline

Current status: **in progress**.

Completed:

- Classic `G...` and contract `C...` account identity remain distinct from signer capability;
- exact Classic transaction XDR/network signing remains the established boundary;
- Protocol-28-ready `stellar-xdr` is used ahead of Mainnet activation;
- Soroban authorization parsing/preimage/hash/signature primitives support legacy Address and CAP-71 AddressV2;
- direct G-account Ed25519 Soroban authorization signing is supported;
- C-account/custom/delegated authorization remains fail closed/provider-owned;
- language-neutral Soroban authorization signing vectors exist;
- `CoreClientApi` exposes protected and external Ed25519 auth-entry signing with stable `invalid-authorization` semantics.

Next:

- complete the platform-neutral SDK contract for Soroban authorization;
- add standard message-signing semantics (SEP-53 alignment) as a separate signing domain;
- keep protocol-version/network feature gating above Core where network state is known.

### M1 — SDK adaptation

Expose stable, domain-specific APIs rather than a generic hash-signing oracle:

```text
Classic transaction signing
Soroban authorization signing
standard message signing
external signing prepare/apply
verification
```

Native/Process/WASM bindings follow only after the platform-neutral SDK contract is proven.

### M2 — reference semantics

RefPython should lead Soroban wallet semantics that are not cryptographic authority:

- RPC simulation and assembly lifecycle;
- review-before/after-simulation integrity;
- invocation/auth-entry presentation;
- G-account fee/source versus C-account authorization relationships;
- stale simulation/error behavior.

RustClient should then provide the Rust reference implementation for RPC/gateway, simulation, contract invocation and wallet capability composition.

### M3 — concrete providers

Smart-account/passkey/contract-account support should start with one real provider and conformance evidence, then extract a generic provider boundary. Do not design a universal provider abstraction from hypothetical use cases.

## Parallel security tracks

These remain important but should not displace the Modern Stellar Core baseline:

- Native SDK release supply-chain pinning, dependency audit, SBOM and provenance;
- Backup v1 metadata-mutation regressions and terminal/legacy containment;
- Process Binding privilege-profile review before non-RefPython Desktop use;
- hardware/external signer transports only with concrete provider demand.

## Agent integration

Fresnica does not build a parallel Agent wallet stack. Soneso Stellar Agent Wallet remains the preferred MCP/policy/approval/audit/network layer. Fresnica contributes a protected exact-envelope signing backend only after the upstream seam exists.

The dormant operation-type `AgentCapability` is not a product policy engine and must not be promoted.

## CI and repository governance

Development uses layered validation:

- portable rustfmt locally before push;
- `Required CI / validate` for affected Core/SDK/direct Rust contracts;
- expensive platform/integration workflows only for non-draft/final PR validation;
- Main bundle after merge for isolated development recovery.

Repository branch protection/ruleset should eventually require PRs and `Required CI / validate`, prohibit force-push/deletion, and require current branches/conversation resolution.

# Fresnica Roadmap

Updated: 2026-09-03

Fresnica is a shared Stellar wallet/security foundation. `Fresnica/fresnica` is converging on one clear responsibility: **Core / SDK / Specification / Reference**. Product UI/application code belongs in independent repositories.

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
   RefPython              Rust Capability Reference
semantic laboratory        reference/rust-client
       |                          |
       +------------+-------------+
                    |
         Capability contracts/evidence
                    |
       +------------+-------------+
       |            |             |
       v            v             v
    Mobile       Terminal      Web/Desktop/Agent
 independent    independent       independent
   product        product           products
```

The protocol target remains [`development/modern-stellar-core-capability-baseline.md`](development/modern-stellar-core-capability-baseline.md).

## Repository boundary cleanup

Repository cleanup status:

1. Core, SDK, bindings, specifications and reference implementations stay here;
2. `reference/python` remains the readable semantic/protocol laboratory;
3. `reference/rust-client` remains the reusable Rust Capability reference;
4. Rust CLI + TUI are extracted together into [`Fresnica/fresnica-terminal`](https://github.com/Fresnica/fresnica-terminal) with pinned shared dependencies and independent CI;
5. terminal product source/workflows are removed from this shared repository;
6. product-specific hardware/UI/provider mechanics stay in product/reference layers rather than moving into Core.

## Modern Stellar foundation

### M0 — Core protocol/security baseline

Current status: **in progress**.

Completed:

- Classic `G...` and contract `C...` account identity remain distinct from signer capability;
- exact Classic transaction XDR/network signing remains the established boundary;
- Protocol-28-ready `stellar-xdr` is used ahead of Mainnet activation;
- Soroban authorization parsing/preimage/hash/signature primitives support legacy Address and CAP-71 AddressV2;
- direct G-account Ed25519 Soroban authorization signing is supported;
- final SEP-53 v1.0.0 message signing/verification is a separate Core/SDK domain with shared cross-language vectors and a concrete Native/React Native Mobile path;
- C-account/custom/delegated authorization remains fail closed/provider-owned;
- language-neutral Soroban authorization signing vectors exist;
- `CoreClientApi` exposes protected and external Ed25519 auth-entry signing with stable `invalid-authorization` semantics.

Next Core-domain work:

- keep protocol-version/network feature gating above Core where network state is known;
- add C-account/smart-account primitives only when a concrete provider proves the required boundary.

### M1 — SDK adaptation

The SDK already exposes domain-specific Classic transaction signing, SEP-53 message signing, Soroban authorization signing and external prepare/apply/verify semantics. Process Binding v2 carries the Soroban authorization contract for the concrete RefPython consumer.

Native/WASM expansion remains consumer-driven. Do not add bindings merely for matrix symmetry.

### M2 — reference semantics

RefPython has proven Soroban simulation/assembly/review, source-account and detached Classic G-account authorization/signing/submission, plus physical Ledger Classic clear-signing through the External Signer boundary.

`reference/rust-client` provides the Rust reference implementation for RPC/gateway, Soroban lifecycle and wallet capability composition.

### M3 — concrete providers

Smart-account/passkey/contract-account support should start with one real provider and conformance evidence, then extract generic semantics. Do not design a universal provider abstraction from hypothetical use cases.

Ledger is the first hardware-signer evidence: physical macOS/Testnet clear signing succeeded with Stellar app 6.0.3 while Blind Signing was disabled. This proves the provider-neutral Core boundary; it does not imply universal Ledger-model certification.

## Parallel security tracks

These remain important but should not displace the Modern Stellar Core baseline:

- Native SDK release supply-chain pinning, dependency audit, SBOM and provenance;
- Backup v1 metadata-mutation regressions and terminal/legacy containment;
- Process Binding privilege-profile review before non-RefPython Desktop use;
- product hardware adapters only where a concrete consumer needs them.

## Agent integration

Fresnica does not build a parallel Agent wallet stack. Soneso Stellar Agent Wallet remains the preferred MCP/policy/approval/audit/network layer. Fresnica contributes a protected exact-envelope signing backend only after the upstream seam exists.

The dormant operation-type `AgentCapability` is not a product policy engine and must not be promoted.

## CI and repository governance

Development uses layered validation:

- portable rustfmt locally before push;
- `Required CI / validate` for affected Core/SDK/binding/reference contracts;
- product/integration workflows only for the affected product/reference surface;
- Main bundle after merge for isolated development recovery.

Repository rulesets are active on the default branch: PRs and the stable required CI are mandatory, merge history is squash/linear, signed commits are required, conversations must resolve, and force-push/deletion are prohibited. Historical development branches remain separate cleanup debt.

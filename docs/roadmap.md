# Fresnica Roadmap

Updated: 2026-08-31

Fresnica has a stable multi-platform wallet-security foundation. The shared repository now prioritizes security, protocol correctness and cross-platform contracts; product-host implementation belongs to independent products.

## Architecture

```text
Application Flows
  -> Application Capabilities
  -> Fresnica SDK/Core + Stellar/network/repository/platform ports
```

Core-owned security delivery paths are:

```text
Rust application       -> fresnica-sdk -> Core
Native application     -> Native SDK / UniFFI -> fresnica-sdk -> Core
Web application        -> filtered WASM binding -> fresnica-sdk -> Core
Trusted process host   -> optional Process Binding -> fresnica-sdk -> Core
```

The Process Binding is a privileged owner/host API, not an Agent, remote, renderer or plugin interface.

## Completed foundation

### Common contracts

- canonical Flow / Capability / Core / Port vocabulary;
- five shared architecture, Flow, Capability, Core-security and platform contracts;
- nineteen-capability catalog with maturity and Reference Semantics rules;
- separate Account identity, Signer capability and Recovery Source concepts;
- shared network, asset, amount, price, memo and error semantics;
- RefPython laboratory governance and cross-repository evidence process.

### Rust Core and SDK

- Stellar Classic account parsing and SEP-0005 mnemonic/signer derivation;
- Scrypt + AES-256-GCM protected software-signer envelope;
- verified `WalletUnlockKey` derivation and routine signing;
- passphrase-only Reveal / Export;
- external Ed25519 prepare/apply with signature verification;
- bounded XDR parsing and stable security error classes;
- Core Client API v3 and Universal SDK API v3;
- Native SDK / UniFFI API v2, WASM binding and Process Binding API v1;
- SDK compatibility manifest, release contract and `native-sdk-v0.2.1` baseline.

### Application capability references

- reusable Rust wallet, balance, Payment, Trustline and transaction flows;
- SDEX write/read semantics including BUY/SELL intent and exact rational prices;
- Ledger Authorization and Signing Coordination for supported local Ed25519 paths;
- Rust CLI and engineering/reference TUI over the same capability layer;
- pending/uncertain-submission and post-success refresh isolation.

### Network and Anchor

- shared `HorizonGateway` boundary with provider normalization;
- staged Horizon-to-RPC/Portfolio transition rationale;
- centralized Anchor HTTPS/no-redirect/DNS/timeout/body-limit policy;
- SEP-1, Classic SEP-10, SEP-24-preferred / SEP-6-fallback initiation and status;
- common SEP-12 scalar/binary updates and reviewed withdrawal handoff;
- exact-case asset identity and safe legacy SEP-6 compatibility.

### Platform delivery

- Android AAR and Apple XCFramework Native SDK packages;
- Swift/Kotlin generation and direct-consumer validation;
- React Native adapter build/consumer gates;
- WASM package/runtime conformance;
- real Testnet/WebAuthn smart-account reference evidence;
- retired legacy `bindings/mobile` facade, publisher and active compatibility CI.

## Current security track

The focused review is recorded in [`development/security-review-2026-08-31.md`](development/security-review-2026-08-31.md).

### 1. Agent Access — design before exposure

The current Core `AgentCapability` is dormant and has no production consumer. Its operation-type/fee/count/expiry checks do not constrain destination, asset, amount/value, transaction timebounds or stateful replay/budget use.

Next milestone:

- define the threat model and credential/grant lifecycle;
- separate deterministic Core transaction-policy evaluation from stateful revocation, nonce, use-count and budget accounting;
- begin with one narrow operation-specific policy;
- preserve exact-envelope authorization and signing;
- deny unsupported transaction/operation forms;
- add negative destination/asset/amount/fee/time/source/replay tests before adding any adapter.

### 2. Release supply-chain hardening

The Native SDK release path publishes wallet-security binaries and therefore needs stronger reproducibility and provenance.

Next milestone:

- SHA-pin third-party Actions;
- pin Fresnica-owned release toolchain/dependency resolution;
- reduce `contents: write` to the final publish job;
- add dependency policy/audit;
- generate an SBOM;
- attach build provenance/attestation to release artifacts.

This applies to Fresnica-owned build inputs, not consumer product Gradle/Kotlin/JDK policy.

### 3. Backup v1 containment and regression coverage

Backup v1 protects signer material but does not authenticate the outer wallet relationship metadata.

Next milestone:

- keep v1 terminal/reference and legacy-only;
- add network/address/relationship mutation tests, including empty-install restore;
- preserve Backup/Restore v2 explicit target-network confirmation and revalidation-before-activation;
- harden user-selected backup temporary-file creation against predictable sibling `.tmp` races.

### 4. Process Binding privilege review

Process Binding API v1 includes owner-only mnemonic generation, Reveal and raw unlock-key derivation.

Next milestone:

- keep RefPython/conformance use trusted;
- prohibit direct remote/MCP/renderer/plugin exposure;
- before a non-RefPython Desktop consumer ships, decide whether it needs the full owner API or a narrower profile.

## Demand-driven work

Do not implement these merely to close a checklist:

- hardware/external signer transports without a concrete provider;
- Hash-X/signed-payload collection without a real flow;
- SEP-45 execution or uncommon SEP-12 file workflows without a concrete anchor;
- platform-native product passkey wiring;
- Windows/Linux non-Rust packaging without a chosen consumer framework;
- product persistence, onboarding or UI in this shared repository.

## Validation and governance

- PR changes run the relevant Required CI and platform gates.
- `main` publishes a verified Main bundle for isolated development recovery.
- API/version compatibility and conformance fixtures remain authoritative.
- Repository branch protection/ruleset should require PRs and `Required CI / validate`, and prohibit force-push/deletion.

## Next checkpoint

A security milestone is complete only when:

1. the threat/compatibility boundary is documented;
2. executable negative regression tests reproduce the unsafe case;
3. the smallest corrective implementation passes the relevant Core/SDK/platform gates;
4. handoff, tasks and security-review status are updated together.

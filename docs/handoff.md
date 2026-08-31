# Fresnica Project Handoff

Updated: 2026-08-31

This is the continuation/state document for the shared Fresnica repository. Stable architecture and security rules live in the common contracts; start at the [repository README](../README.md) and [`docs/README.md`](README.md). Verify GitHub `main`, current CI and release metadata before development rather than treating a handoff SHA as permanent truth.

## 1. Canonical architecture

```text
Application Flows
  -> Application Capabilities
  -> Fresnica SDK/Core + Stellar/network/repository/platform ports
```

- **Flow** owns product sequence, confirmation and UI/UX.
- **Capability** owns shared wallet/application semantics.
- **Fresnica Core** is the Rust cryptographic/security authority.
- **Ports/infrastructure** own platform, network and storage mechanisms.

The five common contracts remain authoritative:

1. [`architecture.md`](architecture.md)
2. [`application-flows.md`](application-flows.md)
3. [`application-capabilities.md`](application-capabilities.md)
4. [`core-security-boundary.md`](core-security-boundary.md)
5. [`platform-implementation.md`](platform-implementation.md)

Independent products consume these contracts and may contribute evidence-backed contract changes. Product implementation work does not belong in this shared repository merely because a checklist mentions a platform.

## 2. Current Core / SDK baseline

The Core/SDK foundation is established and should remain stable unless a concrete security, protocol or consumer-compatibility need requires change.

```text
Native SDK release       native-sdk-v0.2.1
Native Binding API       2
Universal SDK API        3
Core Client API          3
Process Binding API      1 (pre-release)
RN adapter source        0.2.1
```

Current supported integration paths are:

```text
Rust application       -> fresnica-sdk -> Core
Native application     -> Native SDK / UniFFI -> fresnica-sdk -> Core
Web application        -> filtered WASM binding -> fresnica-sdk -> Core
Trusted process host   -> Process Binding -> fresnica-sdk -> Core
```

PR #122 simplified these boundaries:

- the optional Process Binding is the single SDK-level subprocess adapter;
- duplicate Core/SDK bridge binaries were retired;
- Payment, Trustline and SDEX asset identity share a thin wrapper over `stellar_xdr::Asset`;
- the frozen legacy `bindings/mobile` facade and active compatibility CI were retired;
- Anchor HTTP transport policy was centralized;
- the current network adapter is explicitly `HorizonGateway`, with staged RPC/Portfolio migration documented;
- RefPython is governed as an executable product-semantics laboratory rather than a second security authority.

Historical `mobile-sdk-v0.1.0` and `native-sdk-v0.1.0` artifacts remain compatibility evidence, not new-project baselines.

## 3. Security model that must not regress

The critical identity rule remains:

```text
Account identity != Signer capability != Recovery source
```

Consequences include:

- watch-only reads require no secret, mnemonic or wallet passphrase;
- attaching signing material verifies derived signer identity before persistence mutation;
- detaching a signer preserves account identity;
- a `C...` account identity is not an Ed25519 signer key;
- routine signing and Reveal/Export have different authorization privilege;
- System Auth is device-local authorization, not a wallet encryption format;
- ordinary application/JS state must not receive private keys, mnemonics or native unlock keys.

### Process Binding

The Process Binding is a **privileged owner/host surface**. API v1 transports passphrases, mnemonics, secrets, Reveal results and `WalletUnlockKey` values over one-shot stdin/stdout. It must not be exposed directly as a remote service, MCP tool, browser/renderer API, untrusted plugin interface or Agent Access API. A future Agent Access surface must be narrower and must not inherit owner-only Reveal, key-derivation or passphrase operations.

### Agent Access

The current Core `AgentCapability` is dormant and has no production SDK/binding/transport consumer. Its operation-type/fee/count/expiry checks are useful prototype evidence but are insufficient authorization for autonomous signing because they do not constrain destination, asset, amount/value or operation-specific execution semantics. Do not expose it. Replace it with transaction-specific policy after a threat model and negative regression suite.

### Known open security work

1. Pin and harden the Fresnica-owned release supply chain; add dependency audit, SBOM and provenance/attestation without forcing consumer product toolchains.
2. Keep terminal Backup v1 legacy-only because outer wallet metadata is not authenticated; portable products should use the v2 relationship/revalidation model.
3. Review the privileged Process Binding before using it outside RefPython/conformance hosts.
4. Design transaction-specific Agent Access before any AI/agent signing exposure.

## 4. Capability and reference status

The catalog remains **19 Capabilities: 9 Normative and 10 Defined**. See [`application-capabilities.md`](application-capabilities.md) and [`capabilities/README.md`](capabilities/README.md).

The Rust `clients/rust-client` crate is a reusable reference implementation for CLI/TUI; it is not mandatory runtime code for Mobile, Web or Desktop. RefPython leads uncertain product semantics through the documented Experimental -> Candidate -> Normative -> Implemented path, while cryptography, ABI, protocol security fixes and platform security boundaries remain owned by their authoritative layers.

See [`development/refpython-laboratory.md`](development/refpython-laboratory.md).

## 5. Network / Anchor direction

`HorizonGateway` is the current provider adapter, not a permanent shared contract. Provider JSON should terminate at the gateway/normalization boundary. Endpoint families may move to RPC, Portfolio or another history provider only when equivalent semantics, network identity and uncertain-submission behavior are preserved.

Anchor currently covers SEP-1, Classic SEP-10, SEP-24-preferred / SEP-6-fallback transfer initiation, transaction status, reviewed withdrawal handoff and common scalar/binary SEP-12 updates. SEP-45 execution, uncommon nested SEP-12 values/files and provider-backed external signer collection remain demand-driven.

## 6. Validation baseline

PR #122 passed the repository's full pull-request matrix, including Core, SDK, Process Binding, Rust client/CLI/TUI, RefPython, Native/Apple/Android packaging, React Native adapter, WASM and compatibility gates. The resulting `main` successfully produced Main bundle run #46.

The Main bundle is the preferred GitHub-to-isolated-development baseline. Do not upgrade static analysis or formatting into a claim that compilation/platform validation passed; cite the actual workflow that ran.

## 7. Immediate next work

The shared repository should now prioritize security and protocol correctness rather than product-specific implementation:

1. keep this handoff/tasks baseline current after architectural changes;
2. perform a focused security verification pass using source review, CodebaseMemory call tracing and executable regression/PoC tests;
3. harden the active release supply chain;
4. lock Backup v1 to legacy/reference use and test metadata-tampering behavior;
5. threat-model and design transaction-specific Agent Access, then implement only a narrow first operation family;
6. keep hardware/provider transports, uncommon SEP extensions and extra platform packages demand-driven.

## 8. Start here next session

1. Verify GitHub `main`, CI and release state.
2. Restore the newest successful Main bundle when isolated code execution is needed.
3. Read the five common contracts and the relevant Capability file.
4. For security work, read [`core-security-boundary.md`](core-security-boundary.md), [`sdk/process-binding.md`](sdk/process-binding.md), and the affected Core/SDK source.
5. Use CodebaseMemory for orientation and call/blast-radius analysis, then verify every important conclusion against source.
6. Preserve exact-review/exact-sign semantics and fail closed on unsupported transaction forms.

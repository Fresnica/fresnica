# Fresnica Security Review — 2026-08-31

Status: **focused static and call-path review**, not a complete professional audit.

Reviewed baseline: GitHub `main` at `73c2d62e3ec5165675aba45e8eba144400374867` after PR #122. The matching Main bundle artifact was verified before review.

## Method

- built a disposable CodebaseMemory index from the reviewed source: 6,421 nodes and 30,095 relations;
- used the graph for symbol discovery, caller/callee tracing and consumer inventory;
- verified every material conclusion against source code and tests;
- reviewed Agent Access, Backup v1 restore/write paths, Process Binding operations, Core signing/protection paths and Native SDK release automation.

CodebaseMemory is orientation evidence, not proof of completeness. The Objective-C React Native shim remains the one known partial parse; none of the findings below depend on that file.

## Findings

### S1 — Dormant `AgentCapability` is not safe autonomous transaction authority

Severity: **High if exposed; currently dormant**.

The current capability binds one Classic account and network to:

- an `OperationType` allowlist;
- maximum operation count;
- total transaction-fee ceiling;
- optional signing-time expiry.

`authorize_agent_transaction` checks the envelope/source/signature state and operation discriminants, but it does not inspect operation payloads. A capability allowing `Payment` therefore does not constrain destination, asset or amount. Other operation types likewise lack operation-specific execution limits.

Expiry is checked against `now_unix` when authorization/signing occurs. The code does not require the Stellar transaction's own timebounds to end at or before capability expiry. A signed envelope can therefore remain valid under Stellar transaction rules after the capability itself has expired.

CodebaseMemory and source search found no SDK, Native, Process, CLI, RefPython or product consumer. Only Core tests call `authorize_agent_transaction` / `sign_agent_transaction`. This is the right point to replace the prototype rather than preserve its API shape.

Required boundary before exposure:

1. do not export the current operation-type allowlist through SDK/bindings/agent tools;
2. define operation-specific policy, beginning with one narrow operation family;
3. bind transaction timebounds to grant expiry;
4. separate deterministic Core policy evaluation from stateful grant, nonce, revocation and budget accounting;
5. require exact-envelope review/sign binding;
6. add negative tests for destination, asset, amount/value, fee, time, source, operation count, replay and unsupported operation forms.

### S2 — Backup v1 authenticates signer material, not the outer wallet relationship

Severity: **Medium; confirmed legacy-format limitation**.

Backup v1 stores a complete outer `WalletRecord` beside the protected signer envelope. Fields such as account address, wallet type, network and metadata are outside the signer's AEAD envelope.

Both Rust CLI and RefPython restore paths verify the protected signer against the current application passphrase only when an application passphrase already exists. On an empty installation, a non-watch-only v1 record can be persisted before signer/relationship verification. Later signing paths still perform signer identity checks, but the v1 network field is not authenticated by the protected signer envelope.

Decision:

- keep v1 terminal/reference backup legacy-only;
- do not promote v1 as a portable product format;
- add metadata-mutation regressions, especially network mutation and empty-install restore;
- portable restore uses the v2 Account/Signer relationship model with explicit target-network confirmation and revalidation before activation.

### S3 — Native SDK release inputs are not yet fully reproducible or attestable

Severity: **Medium supply-chain risk**.

The release workflow can publish wallet-security SDK binaries with `contents: write`. The reviewed repository currently uses mutable Action/toolchain references such as `actions/checkout@v4` and `dtolnay/rust-toolchain@stable`; it carries no repository Rust toolchain file or Cargo lockfile. The release produces checksums, but no dependency-audit, SBOM or provenance/attestation gate is present.

The broader workflow set contains many mutable major-version Action references. The release workflow is the highest-priority path because its output is published for downstream applications.

Recommended sequence:

1. SHA-pin third-party Actions in Fresnica-owned workflows;
2. pin the release Rust toolchain and release dependency resolution;
3. grant write permission only to the final publish job;
4. add dependency policy/audit;
5. generate an SBOM;
6. publish artifact attestations/provenance tied to the source commit and release artifacts.

Consumer product Gradle/Kotlin/JDK policy remains outside this repository; only Fresnica-owned build inputs should be pinned here.

### S4 — Process Binding is a privileged owner interface, not an agent sandbox

Severity: **Boundary/integration risk**.

Process Binding API v1 intentionally includes owner-sensitive operations such as mnemonic generation, Reveal and raw unlock-key derivation. Stdin/stdout avoids argv/environment leakage, but the binary does not authenticate its parent and does not isolate an untrusted renderer, plugin or remote caller.

It may be used by a trusted RefPython or managed host process. It must not be exposed directly as:

- a network daemon;
- MCP/agent tooling;
- an Electron/browser renderer API;
- an untrusted plugin interface;
- a shared multi-tenant service.

A future Agent Access surface must be separate and must not inherit passphrase, Reveal, secret/mnemonic or raw-unlock-key operations.

### S5 — Backup atomic writes use a predictable sibling `.tmp` path

Severity: **Low local hardening item**.

Rust and RefPython backup writers use a deterministic `<destination>.tmp` sibling before rename/replace. In an attacker-controlled directory this leaves a local symlink/race surface. Normal wallet storage is directory-restricted, but user-selected backup destinations need not be.

A future hardening should use an exclusively created randomized temporary file in the destination directory, reject unsafe file types/links and preserve the current restrictive permissions before atomic replacement.

## Positive observations retained

This pass did not find a direct private-key disclosure, fixed AES-GCM nonce, weak RNG use, arbitrary signature acceptance or mutation-before-signature-verification flaw in the reviewed Core paths.

The reviewed implementation continues to:

- use Scrypt-derived keys and AES-256-GCM for protected signer material;
- generate salts/nonces through the operating-system random source;
- wrap sensitive buffers with zeroizing types in key paths;
- recompute the Stellar transaction hash from the exact envelope and network passphrase;
- verify an external Ed25519 signature before mutating the envelope;
- reject duplicate signatures and unsupported envelope forms in the relevant paths;
- bound XDR parsing depth.

These observations do not replace fuzzing, transitive dependency audit, malicious XDR/backup corpora, platform runtime instrumentation or external review.

## Recommended order

1. design and replace dormant Agent Access before any exposure;
2. harden the Native SDK release supply chain;
3. add Backup v1 mutation regressions and keep v1 legacy-only;
4. review whether non-RefPython Process Binding consumers need a narrower owner profile;
5. harden user-selected backup temporary-file creation.

# Fresnica Modern Stellar Core Capability Baseline

Status: **Target baseline for staged evolution**  
Date: **2026-09-01**  
Scope: **Fresnica shared foundation — Core, SDK, conformance vectors, then reference wallet implementations**

## 0. Decision

Fresnica should evolve from a strong **Classic-first Stellar wallet foundation** into a **modern Stellar security/signing foundation** that understands the protocol-level identity and authorization primitives required by both Classic and Soroban.

The goal is **not** to turn Fresnica Core into a full Stellar SDK, RPC client, smart-contract framework, DeFi stack, or product wallet.

The target layering is:

```text
Stellar protocol / CAP / stable ecosystem standards
                    |
                    v
            Fresnica Protocol/Core
       slow-moving security authority
                    |
                    v
               Fresnica SDK
       stable cross-language semantics
                    |
          +---------+----------+
          |                    |
          v                    v
      RefPython            Rust Client
 semantic laboratory      wallet reference
          |                    |
          +---------+----------+
                    |
       Mobile / Desktop / Web / Agent
              independent products
```

The governing rule is:

> **Core tracks protocol and security primitives. SDK tracks stable semantic access. Wallet/reference layers track application capabilities. Products own UX, policy, persistence, platform authorization and orchestration.**

This baseline is intended to guide several development cycles. It is not a requirement to implement every item in one release.

---

# 1. Why this baseline exists

Fresnica's current architecture already has the right foundational separation:

```text
Account identity != Signer capability != Recovery source
```

The current Core can:

- parse `G...` Classic account identities;
- parse `C...` contract account identities without pretending they have an Ed25519 public key;
- protect S-key and mnemonic-backed software signers;
- derive and validate protected unlock material;
- hash and sign Stellar transaction envelopes using the exact XDR plus network passphrase;
- verify external Ed25519 signatures before mutating an envelope;
- preserve transaction XDR and network context when handing signing work to an external signer.

This was a good security foundation but was **application-facing Classic-centric when this baseline was first written**. The first Core Soroban authorization slice and its `CoreClientApi` exposure are now implemented; the platform-neutral SDK adaptation remains the next active step. Modern Stellar adds a second authorization world:

```text
Classic transaction authorization
    G-account transaction signatures
    Classic signer weights / thresholds

Soroban authorization
    G or C address authorization
    SorobanAuthorizationEntry
    invocation trees
    nonces
    ledger-based expiration
    contract-defined __check_auth
    delegated/address-bound credentials
```

Fresnica must understand this distinction at the foundation level or every future Mobile/Web/Desktop/Agent implementation will be forced to invent its own Soroban security boundary.

---

# 2. Reviewed external baseline — 2026-09-01

This document distinguishes **network-active protocol semantics** from **available library/XDR types**.

## 2.1 Network protocol

As reviewed on 2026-09-01:

- Stellar Mainnet is on **Protocol 27**.
- Protocol 27 includes CAP-71: authentication delegation and address-bound Soroban credentials.
- Protocol 28 (Adapter) stable infrastructure/SDK releases are available for pre-upgrade integration. SDF scheduled the Testnet upgrade vote for 2026-08-27 and the Mainnet upgrade vote for 2026-09-16. Fresnica should implement and validate official Protocol 28 semantics ahead of Mainnet activation rather than waiting for the vote.

Therefore:

> **A type being present in `stellar-xdr` does not prove that the connected network has activated the corresponding protocol feature.**

Fresnica should proactively compile against and implement maintained/current Protocol 28 XDR/semantics before Mainnet activation. Runtime/application code must still distinguish implementation readiness from the target network's active protocol: where a feature is not yet active, emitting it must remain gated or fail closed.

## 2.2 Relevant protocol and ecosystem standards

Priority is determined by status and layer:

| Standard | Status / role | Fresnica relevance |
| --- | --- | --- |
| CAP-71 / 71-01 / 71-02 | Final, Protocol 27 | Core authorization semantics; AddressV2 and delegation recognition |
| Stellar XDR | protocol schema | Core parsing, hashing, exact wire identity |
| Stellar RPC `simulateTransaction` | network API | RustClient/RefPython transaction assembly; **not Core** |
| SEP-53 | Final | standard message signing/verification security primitive |
| SEP-43 | Draft v1.2.1 | useful wallet API alignment target: `signTransaction`, `signAuthEntry`, `signMessage`, `getNetwork` |
| SEP-45 | Draft | contract-account web authentication; consumes Soroban authorization capability |
| SEP-41 | Draft, broadly used token interface | Wallet/contract-token semantics; not Core custody/signing logic |

Draft SEPs are implementation/alignment evidence, not authority to freeze a Fresnica Core API prematurely.

---

# 3. Layer ownership

## 3.1 Fresnica Protocol/Core

Core owns only deterministic, security-relevant behavior that should remain correct regardless of product host:

- Stellar address/account identity parsing where identity affects cryptographic behavior;
- network ID derivation from network passphrase;
- bounded XDR parsing/encoding for supported security objects;
- protocol-defined signing preimages/hashes;
- protected signing material lifecycle;
- signer identity verification;
- transaction signature production and verification;
- Soroban authorization signature production and verification for explicitly supported signer forms;
- standard prefixed-message signing/verification when adopted;
- exact-XDR / exact-authorization-entry integrity;
- fail-closed handling of unknown or unsupported credential/signature forms.

Core does **not** own:

- RPC/Horizon access;
- `simulateTransaction`;
- resource/fee estimation;
- sequence fetching;
- contract spec retrieval;
- contract method interpretation;
- token catalog/provider ranking;
- smart-account deployment;
- application approval flows;
- system biometric/passkey UI;
- persistent wallet repositories;
- Agent policy/budget/audit engines;
- DeFi protocols.

## 3.2 Fresnica SDK

SDK exposes the stable, platform-neutral semantic access to Core.

SDK should make protocol signing domains explicit rather than expose a generic hash-signing oracle.

Target families:

```text
Account / Address identity
Protected signer lifecycle
Classic transaction signing
Soroban authorization signing
Standard message signing
External signing prepare/apply
Verification
```

SDK remains stateless with respect to application sessions, network fetching, persistence and OS authorization.

## 3.3 RefPython

RefPython remains the **semantic laboratory**, not a second cryptographic authority.

It should lead experiments around:

- contract invocation lifecycle;
- simulation and assembly;
- review after simulation;
- G-account versus C-account authorization flows;
- fee-payer versus authorizer separation;
- stale simulation / stale authorization behavior;
- contract/token display semantics;
- error and recovery states;
- product-level Soroban payment/invocation flows.

It must call SDK/Core for security-owned signing behavior.

## 3.4 Rust Client

`reference/rust-client` remains a wallet/reference implementation and may evolve more quickly than Core.

It may own/reference:

- `HorizonGateway` for remaining Classic/history paths;
- a first-class `RpcGateway` for modern Stellar/Soroban paths;
- simulation;
- transaction assembly;
- contract invocation;
- authorization coordination;
- SAC and contract-token portfolio projection;
- events/history normalization;
- submission and uncertainty reconciliation.

It is not mandatory runtime code for other products.

---

# 4. Identity model baseline

## 4.1 Keep G and C as different account identities

The current Core rule is correct:

```text
G... -> Classic account, contains Ed25519 account public key identity
C... -> Contract account, no implicit Ed25519 public key
```

Do not introduce `contract_public_key` or derive a signing key from a C-address.

For contract accounts:

```text
C-account identity
    != authentication implementation
    != signer/provider
    != passkey
    != Ed25519 key
    != smart-account policy
```

A C-account's authorization semantics are defined by contract logic such as `__check_auth()`.

## 4.2 Introduce a clear authorization-address concept

Soroban authorization operates over `SCAddress`, where G and C addresses participate in the same address-oriented authorization model.

The foundation should distinguish:

```text
AccountIdentity
    G or C account identity

AuthorizationAddress / StellarAddress
    address participating in Soroban authorization context

SignerIdentity
    actual invokable cryptographic/provider identity
```

Exact public type names should be chosen during implementation, not frozen by this document.

## 4.3 Muxed accounts are not ledger account identities

`M...` addresses must be supported where Stellar permits them, but they must not be treated as a third on-ledger account type.

A muxed address is:

```text
underlying G account + 64-bit muxed ID
```

The underlying G account is the ledger identity and signer authority.

Therefore:

- do not broaden `AccountIdentity` into `Classic | Contract | Muxed` merely because M is a StrKey;
- model M as a destination/source-routing value at the appropriate transaction/payment layer;
- expose conversion to its underlying G identity where authorization requires it.

---

# 5. Signing-domain baseline

Modern Fresnica should recognize **three distinct standard signing domains**, each with its own API and validation rules.

## 5.1 Classic transaction signing — already established

Conceptually:

```text
exact TransactionEnvelope XDR
+ network passphrase
+ expected signer identity
-> protocol transaction hash
-> signer
-> verify returned signature
-> append DecoratedSignature
```

Current invariant remains:

> The external provider receives the exact transaction XDR and network context; Fresnica is not reduced to an arbitrary 32-byte signing oracle.

## 5.2 Soroban authorization-entry signing — highest-priority gap

Core/SDK must add a dedicated Soroban authorization domain.

An authorization signing request must preserve enough public context for a security provider to know exactly what it is authorizing, including where relevant:

- exact authorization entry/preimage XDR;
- network passphrase/network ID;
- authorizing G or C address;
- credential type;
- nonce;
- signature expiration ledger;
- complete authorized invocation tree;
- protocol/preimage variant required by the credential type.

Conceptual target:

```text
Soroban auth XDR
+ network
+ authorizer address
+ exact invocation tree
+ nonce
+ expiration ledger
-> protocol-defined auth preimage/hash
-> signer/provider
-> verify/apply authorization material
```

This must be a different entry point from transaction signing.

## 5.3 SEP-53 prefixed-message signing — established Core/SDK domain

Message signing is neither transaction signing nor Soroban authorization.

Core/SDK provide a standard SEP-53 v1.0.0 path rather than generic arbitrary-byte Ed25519 signing. The contract preserves the exact source message bytes, encoded prefixed payload, and single-SHA-256 digest; external signers receive that full request and returned signatures are verified before acceptance.

Native Binding API 3 exposes this domain for the concrete Mobile dapp consumer. The React Native adapter accepts JavaScript strings as exact UTF-8 bytes and keeps system-auth/passcode authorization native-only. SEP-53 itself does not add network/session/origin/replay context; those remain Dapp capability semantics.

This makes the common wallet-facing signing set align naturally with the direction of SEP-43:

```text
signTransaction
signAuthEntry
signMessage
```

The SDK does not need to copy SEP-43's JavaScript interface, but it should be able to implement those semantics safely.

---

# 6. Soroban authorization support levels

Do not claim generic C-account/smart-account support after implementing only Ed25519 auth entries.

Use explicit support levels.

## Level A — protocol recognition

Core must be able to safely decode/inspect supported active-protocol authorization forms and fail closed on unsupported ones.

For Protocol 27 this means at minimum recognizing the credential families introduced/retained by the protocol, including:

- source-account credentials;
- legacy address credentials;
- `SOROBAN_CREDENTIALS_ADDRESS_V2`;
- delegated/address-with-delegates credentials.

Recognition does not imply Fresnica can satisfy the authorization.

## Level B — built-in G-account authorization

First concrete signing implementation should support the standard G-account Ed25519 path for non-source detached authorization.

Priority:

1. legacy address credentials for compatibility;
2. AddressV2 credentials for Protocol 27+;
3. prefer AddressV2 when creating new compatible authorization because the signed payload is bound to the authorizer address.

Negative rules:

- wrong network -> reject;
- wrong G signer -> reject;
- mutated nonce -> reject;
- mutated expiration ledger -> reject;
- mutated invocation tree -> reject;
- mutated AddressV2 authorizer address -> reject;
- unsupported credential form -> reject without mutation.

## Level C — external/contract authorization provider

C-account authorization is provider-defined and often produces non-Ed25519 or structured `SCVal` signatures.

Fresnica must not pretend every C-account can be satisfied by `Ed25519Signature`.

Future provider work may support:

- passkeys/WebAuthn;
- OpenZeppelin account contracts;
- hardware-backed mechanisms;
- multisig contracts;
- policy/session signers;
- delegated authorization.

Before a generic provider contract is frozen, at least one real C-account integration should be proven on Testnet.

## Level D — delegated authorization

Protocol 27 delegation must be **recognized now** because it is active protocol surface.

Full collection/production of nested delegated signatures may be deferred until concrete provider evidence exists.

The rule is:

> decode and preserve semantics; do not silently flatten or ignore delegate structure; fail closed when the current provider cannot satisfy it.

---

# 7. Protocol-version policy

Fresnica's current Core dependency uses `stellar-xdr = 28.0.0`, while Mainnet remains Protocol 27 until the scheduled Protocol 28 upgrade vote on 2026-09-16. Proactive Protocol 28 implementation is intentional: the Core should be ready before network activation, not react after it.

This remains safe only with an explicit runtime protocol policy.

## 7.1 Compile capability != network capability

```text
XDR library can decode feature
        !=
connected network has activated feature
```

## 7.2 Network layer owns protocol observation

`RpcGateway` / application infrastructure should obtain network protocol/version information from an authoritative provider such as Stellar RPC `getVersionInfo` or equivalent network configuration.

Core must not make network calls.

## 7.3 Security-sensitive builders must gate activation

When an operation/credential has a minimum protocol version, the application/reference layer must not construct or authorize it for a lower-protocol network.

Where useful, Core/SDK may expose minimum-version metadata or accept a caller-supplied protocol context for deterministic validation, but the exact API should be driven by the first implementation rather than designed abstractly in advance.

## 7.4 Future XDR variants fail closed

Unknown/unsupported union arms, credential types or envelope forms must never degrade into:

```text
"we could parse enough, so sign it anyway"
```

---

# 8. Transaction lifecycle changes for Soroban

Classic application lifecycle remains:

```text
intent
-> prepare exact transaction
-> review
-> authorize
-> sign
-> submit
```

Soroban adds a required simulation/assembly phase:

```text
intent
-> build invocation candidate
-> simulate
-> derive authoritative resources / auth / fees
-> assemble final transaction
-> semantic review of assembled result
-> authorize
-> sign auth entries and/or transaction
-> submit
-> reconcile result
```

## 8.1 Simulation is not a Core responsibility

`simulateTransaction` belongs to RPC/application infrastructure.

It calculates/returns data such as:

- transaction data/footprint;
- resource requirements;
- minimal resource fee;
- required authorization data;
- invocation result/error information.

Core should never fetch or trust RPC itself.

## 8.2 Review must bind to the post-simulation object

The existing Fresnica rule remains authoritative:

> **Review must correspond to the exact thing that will be signed.**

For Soroban this means a product must not:

1. review an unsimulated invocation;
2. simulate/assemble later;
3. silently inherit the old approval after footprint/auth/resource changes.

The review/authorization object must be rebound after simulation/assembly.

## 8.3 Prefer XDR over unstable provider JSON for security identity

The Stellar RPC documentation warns that unpacked JSON schemas may change with underlying XDR.

Security-critical exact identity should therefore remain XDR-based where possible. Provider JSON can be normalized for display/semantics but must not become the canonical object used to prove exact signing identity.

---

# 9. G-account and C-account authorization matrix

| Scenario | Envelope signer | Auth-entry signer | Fee/sequence source |
| --- | --- | --- | --- |
| Classic transaction | G | n/a | G source |
| Soroban source-account authorization | G | implicit through envelope | same G source |
| Soroban detached G authorization | fee/source G may differ | G | transaction source G |
| Soroban C-account authorization | fee/source G required | C-account provider via auth entry | separate G source |

Security consequences:

- C-account must never be offered `sign_transaction_envelope` as if it owned an Ed25519 transaction key;
- fee payer and authorizer are separate identities and must both appear in review/authorization semantics;
- a G-account may authorize a Soroban invocation either through source-account authorization or detached auth entries; these are distinct modes;
- wallet UI/Agent policy must not infer "who authorized" only from transaction source.

---

# 10. Token and asset model

Do not stretch the current Classic `AssetId(stellar_xdr::Asset)` abstraction until it means every form of value on Stellar.

Modern Stellar has at least two important value families:

```text
Classic Stellar Asset
    XLM
    CODE:GISSUER
    native Classic operations / trustlines / SDEX
    optional built-in Stellar Asset Contract (SAC) representation

Contract Token
    C... contract implementing token interface such as SEP-41
    contract invocation semantics
```

The SAC is a bridge between these worlds, not proof that a contract address and a Classic asset identity are interchangeable.

Baseline rule:

> **Keep Classic `AssetId` exact and narrow. Introduce contract-token/value identity at the Wallet Capability layer when real Soroban token flows require it.**

Core only needs the XDR/address primitives necessary for signing and authorization.

RefPython should explore the portfolio/display relationship among:

- Classic asset identity;
- SAC contract identity;
- contract token identity;
- G-account trustline balance;
- C-account/SAC/contract-token balance.

Do not freeze a universal `Asset` type before that evidence exists.

---

# 11. Core API target — semantic sketch, not frozen names

The current `CoreClientApi v3` should evolve only after conformance vectors exist.

A future version will likely need capabilities equivalent to:

```text
parse_account / parse_stellar_identity

prepare_transaction_signing
sign_transaction_xdr
apply_transaction_signature
verify_transaction_signature

prepare_soroban_auth_signing
sign_soroban_auth_entry
apply_soroban_auth_signature
verify_soroban_auth_entry

sign_sep53_message
verify_sep53_message
```

Important constraints:

- do not introduce `sign_hash(bytes32)` as a public general-purpose shortcut;
- transaction and auth-entry methods must remain domain-specific;
- signing requests carry exact public context to external providers;
- returned signatures/authorization material are verified before mutation where verification is possible;
- Core remains free of RPC, storage and system-auth APIs.

---

# 12. SDK target

After Core behavior is fixed by vectors, SDK should expose the same domains in transport-neutral DTOs.

Likely next major/minor API evolution:

```text
SDK v3                 target successor
-------------------------------------------------
parse_account           preserve
protect_*               preserve
derive_unlock_key       preserve
sign_transaction_xdr    preserve
prepare_ed25519_signing preserve
apply_ed25519_signature preserve
                        + Soroban auth prepare/sign/apply
                        + auth verification
                        + standard message signing/verification
```

Versioning rule:

- adding Soroban auth signing should trigger an explicit Core Client API / SDK contract version change;
- Native, Process and WASM bindings adopt the new contract only after shared vectors pass;
- Process Binding remains an owner/trusted-host API, never an Agent API;
- browser/WASM exposure remains filtered and must not expose raw unlock material merely to achieve API symmetry.

---

# 13. Conformance-vector plan

This baseline should be implemented test-first.

## 13.1 Existing vectors retained

- Classic network ID;
- Classic transaction hash;
- decorated Ed25519 transaction signature;
- external signer exact-XDR/network request;
- invalid external signature does not mutate transaction.

## 13.2 New Soroban authorization vectors

Add language-neutral vectors for at least:

### Legacy address authorization

- exact auth-entry XDR;
- Testnet/Mainnet network ID effect;
- nonce;
- expiration ledger;
- invocation tree;
- expected hash/signature;
- signed result.

### Protocol 27 AddressV2

- authorizer address included in signed preimage;
- same key + different authorizer address produces different signing payload;
- wrong address fails verification;
- pre-P27 activation rejected by the application/protocol gate.

### Mutation tests

Changing any of the following after review/preparation must invalidate the signature or be rejected:

- network;
- authorizer address;
- contract address;
- function name;
- invocation arguments;
- nested invocation;
- nonce;
- expiration ledger;
- credential type.

### Unsupported credential tests

- delegated credential decoded but unsupported provider -> typed fail-closed result;
- unknown future credential -> reject;
- malformed recursive XDR -> bounded decode failure.

## 13.3 C-account negative vectors

- C identity has no Ed25519 public key;
- C account cannot enter Classic transaction-envelope signing path;
- generic C auth cannot be satisfied by an Ed25519 software signer unless a concrete contract/provider explicitly defines that relation.

---

# 14. Security invariants

The Modern Stellar work must not weaken the established security model.

## S1 — domain separation

Transaction signing, Soroban authorization signing and message signing are separate public capabilities.

## S2 — exact-context signing

External/provider signing receives the exact public semantic object, not only an opaque digest, unless the trusted provider boundary explicitly owns that reduction internally.

## S3 — Account != Signer

G/C account identity is never proof that local signing capability exists.

## S4 — C != Ed25519

A C-address never implies a private key or transaction-envelope signer.

## S5 — authorizer != fee payer

Soroban authorizer and transaction source/fee payer must remain independently modeled.

## S6 — simulation cannot inherit stale approval

Any simulation/assembly change that affects exact execution or authorization requires a newly bound review/approval.

## S7 — protocol gating

A known future XDR form is not automatically enabled on the current network.

## S8 — provider return verification

Where cryptographically possible, externally produced signatures are verified before mutation/acceptance.

## S9 — fail closed on unsupported auth

Unknown credential/provider/contract-auth semantics never degrade into blind signing.

## S10 — Reveal remains stronger

Adding Soroban does not expose mnemonic, S-key, wallet passphrase, Reveal/Export, or raw unlock material to application/plugin/Agent callers.

---

# 15. Reference implementation roadmap

## Phase 0 — Baseline and inventory — COMPLETE

Goal: agree on semantics before code.

- keep this document as target baseline;
- map current Core/SDK types to the target matrix;
- identify official test vectors/examples for Protocol 27 auth;
- inspect `stellar-xdr 28` feature/current-next behavior;
- document exact protocol activation checks.

Exit criteria:

- no uncertainty about which features belong in Core versus RefPython/RustClient;
- first auth-entry vector can be constructed independently of product code.

## Phase 1 — Core Soroban authorization primitives — COMPLETE

Scope:

- bounded parse/encode of relevant auth-entry/preimage structures;
- protocol-defined auth signing payload/hash generation;
- G-account detached Ed25519 auth support;
- legacy + AddressV2 support;
- signature verification/application;
- unsupported delegated/C-account forms represented and rejected safely;
- no networking.

Exit criteria:

- shared vectors pass;
- wrong network/address/invocation/nonce/expiry tests fail correctly;
- existing Classic vectors remain unchanged.

## Phase 2 — Core/SDK contract evolution — IN PROGRESS

`CoreClientApi` exposure is complete; platform-neutral SDK exposure is the current next slice.

Scope:

- expose auth-entry prepare/sign/apply/verify through Core Client API;
- expose equivalent SDK semantics;
- stable error mapping;
- update compatibility manifest;
- then adapt Native/Process/WASM according to each binding's security profile.

Exit criteria:

- one SDK can safely implement both `signTransaction` and `signAuthEntry` style wallet behavior;
- no generic hash-signing surface was introduced.

## Phase 3 — RefPython Soroban reference flow

Start with the smallest useful Testnet flow:

```text
G account
-> one invokeHostFunction
-> RPC simulation
-> assemble
-> semantic review
-> source-account or detached G auth
-> SDK/Core signing
-> RPC submit
-> status reconciliation
```

Then add a fee-payer/authorizer-separated flow.

Exit criteria:

- simulation-derived changes are visible in review;
- exact assembled XDR and auth entries are bound to signing;
- stale/mutated simulation data is rejected.

## Phase 4 — first concrete C-account provider

Choose one maintained ecosystem implementation, preferably standard-compatible and externally useful.

Candidates may include:

- OpenZeppelin Stellar account contract;
- passkey-based account provider;
- another maintained contract-account implementation with clear Testnet fixtures.

Do not design a universal provider API before completing this integration.

Exit criteria:

- C-account auth entry is produced without pretending C is an Ed25519 key;
- fee payer remains separately modeled;
- provider-specific details do not leak into Core's generic identity model.

## Phase 5 — Rust Client wallet capabilities

Add only after Core/SDK semantics are proven:

- `RpcGateway`;
- simulation/assembly service;
- Contract Invocation reference semantics;
- G/C authorization coordination;
- uncertain submission reconciliation via RPC;
- token/SAC projection based on real flow requirements.

## Phase 6 — standards/product adoption

Use the proven SDK to support or inform:

- SEP-43-compatible wallet surfaces;
- SEP-45 contract-account authentication;
- Mobile/Web/Desktop Dapp Interaction;
- Fresnica/Soneso Agent Wallet signer integration;
- other products.

---

# 16. What we deliberately do not build in this baseline

The Modern Stellar Core baseline does **not** authorize immediate implementation of:

- a Soroswap adapter;
- Blend or DeFindex;
- a generic DeFi layer;
- a Fresnica smart-account contract;
- an OpenZeppelin fork;
- a new MCP server;
- Agent policy/audit/budget machinery;
- a generic contract ABI interpreter in Core;
- WalletConnect transport;
- remote signer service;
- a universal `Asset` abstraction covering every contract token;
- arbitrary message/hash signing;
- every draft SEP.

Those may become product/reference capabilities after the foundation is correct.

---

# 17. Modern Stellar capability matrix

Legend:

- **Established** — current Fresnica foundation already has a meaningful implementation.
- **Foundation gap** — should be added to Core/SDK.
- **Reference gap** — belongs first in RefPython/RustClient.
- **Provider/product** — requires a real external/product implementation before common semantics are frozen.

| Capability | Current Fresnica | Target owner | Target |
| --- | --- | --- | --- |
| G account identity | Established | Core/SDK | preserve |
| C account identity | Established parse only | Core/SDK | preserve and use in authorization context |
| M muxed address | partial wallet-level ecosystem support | Payment/Transaction | routing identity, not ledger AccountIdentity |
| network ID | Established | Core | preserve |
| protocol-version observation | absent in Core by design | Network Gateway | add runtime protocol context |
| Classic transaction hash/sign | Established | Core/SDK | preserve |
| external exact-XDR Ed25519 signing | Established | Core/SDK | preserve |
| Soroban auth-entry parse/hash | Established in Core | Core | preserve + expose through SDK |
| G auth-entry Ed25519 signing | Established in Core/CoreClientApi | Core/SDK | SDK adaptation next |
| AddressV2 | Established in Core | Core/SDK + gateway gating | preserve; CAP-71/P27 active and P28-ready |
| delegated auth recognition | Established recognition; signing rejected | Core | preserve recognition; provider support later |
| C-account auth provider | absent | provider + SDK seam | concrete implementation first |
| SEP-53 signing | Established with final v1.0.0 vectors | Core/SDK; Native for Mobile | preserve separate message domain; product owns challenge/session policy |
| RPC | not Core | RustClient/RefPython | add first-class gateway |
| simulate/assemble | absent | RefPython/RustClient | add after Core auth vectors |
| contract invocation review | absent | Capability/RefPython | define from evidence |
| SAC | Classic Asset model exists | Wallet capability | map without changing Core Asset semantics |
| contract token | absent | Wallet capability | introduce separate identity from Classic Asset |
| SEP-45 | absent | Anchor/Dapp product capability | consume C-account auth support |
| SEP-43 | not a Core interface | product adapter | alignment target, not foundation authority |

---

# 18. Completion definition

The baseline can be considered achieved when all of the following are true:

1. Core treats G and C as first-class but different Stellar account identities.
2. Core supports protocol-correct Classic transaction signing without regression.
3. Core supports protocol-correct Soroban authorization signing for the standard G-account case, including CAP-71 AddressV2 semantics and Protocol 28-ready behavior.
4. Core can recognize active-protocol authorization forms it cannot satisfy and fails closed instead of misclassifying them.
5. SDK exposes separate transaction/auth-entry/message signing domains rather than generic hash signing.
6. Cross-language conformance vectors fix the signing semantics.
7. A RefPython Testnet contract invocation proves simulation -> assembly -> review -> authorization -> signing -> submission.
8. At least one concrete C-account provider proves the external authorization boundary before it is generalized.
9. RustClient gains first-class RPC/Soroban reference behavior without moving networking into Core.
10. Products can implement modern wallet standards such as SEP-43/SEP-45 on top of the same foundation rather than introducing parallel signing/security models.

At that point Fresnica should be describable as:

> **A modern Stellar security and wallet-capability foundation supporting both Classic and smart-contract authorization models, with slow/stable Core evolution and independently evolving application implementations.**

---

# 19. Immediate next development slice

The first Core Soroban authorization slice and `CoreClientApi` exposure are complete. Do not jump directly to RPC/simulation or platform bindings.

The next concrete engineering slice is:

```text
Platform-neutral Soroban Authorization SDK Contract Slice
```

Deliverables:

1. add SDK request/result types for exact auth-entry XDR, preimage XDR, authorization hash and network passphrase;
2. expose protected-signer auth-entry signing through the SDK without exposing raw `WalletUnlockKey`;
3. expose external Ed25519 prepare/apply semantics with the same verification guarantees as Core;
4. map `invalid-authorization` distinctly from `invalid-transaction`;
5. consume the shared Soroban authorization vector in SDK tests;
6. make an explicit compatibility/API-version decision based on the existing manifest rules;
7. keep Native/Process/WASM expansion out of scope until this platform-neutral contract is green.

After this slice, binding adaptation and RefPython simulation/assembly semantics may proceed independently.

---

# 20. References reviewed for this baseline

Official/current references reviewed on 2026-09-01:

- Stellar Software Versions — active/next protocol and SDK versions  
  https://developers.stellar.org/docs/networks/software-versions
- Stellar networks / version information  
  https://developers.stellar.org/docs/networks
- Signing Soroban contract invocations  
  https://developers.stellar.org/docs/build/guides/transactions/signing-soroban-invocations
- Smart contract authorization  
  https://developers.stellar.org/docs/build/guides/auth/contract-authorization
- Contract Accounts  
  https://developers.stellar.org/docs/build/guides/contract-accounts
- `simulateTransaction`  
  https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/simulateTransaction
- Muxed accounts  
  https://developers.stellar.org/docs/build/guides/transactions/pooled-accounts-muxed-accounts-memos
- Stellar Assets and Contract Tokens / SAC  
  https://developers.stellar.org/docs/tokens
  https://developers.stellar.org/docs/tokens/stellar-asset-contract
- CAP-71 — Authentication delegation and address-bound Soroban credentials  
  https://github.com/stellar/stellar-protocol/blob/master/core/cap-0071.md
- CAP-71-02 — Address-bound Soroban credentials  
  https://github.com/stellar/stellar-protocol/blob/master/core/cap-0071-02.md
- SEP-43 — Standard Web Wallet API Interface  
  https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0043.md
- SEP catalog/status  
  https://github.com/stellar/stellar-protocol/blob/master/ecosystem/README.md
- Rust Stellar XDR releases  
  https://github.com/stellar/rs-stellar-xdr/releases

Fresnica source baseline reviewed:

- `core/rust/src/account.rs`
- `core/rust/src/transaction.rs`
- `core/rust/src/signer.rs`
- `core/rust/src/client_api.rs`
- `sdk/rust/src/lib.rs`
- `docs/application-capabilities.md`
- `docs/capabilities/transaction.md`
- `docs/capabilities/ledger-authorization.md`
- `docs/capabilities/dapp.md`
- `docs/development/stellar-agent-wallet-reuse.md`

Repository baseline when this document was drafted:

```text
Fresnica/fresnica main
968b7168c8870658b6631a6f4ae7b5d76c2dd7ab
```

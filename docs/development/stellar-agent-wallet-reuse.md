# Stellar Agent Wallet Reuse and Fresnica Signer Integration

Status: **Accepted integration direction; implementation seam not yet available upstream**

Date: 2026-08-31

## Reviewed baselines

- Fresnica: `4ca40e6e79def9860c724e9bde4b63b96720671e`
- Soneso Stellar Agent Wallet: `fcf1e00ae048363c4a1845f551387767df562ecf`

The external project is currently a public alpha. Its current release posture refuses Mainnet writes and signing, so the first integration is Testnet-only.

## Decision

Fresnica will **not** build a second Agent wallet stack.

The preferred composition is:

```text
Agent / MCP client
        |
        v
Soneso Stellar Agent Wallet
  - MCP stdio transport
  - typed tools
  - policy evaluation
  - operator approval
  - nonce and replay protection
  - stateful budgets and rate limits
  - tamper-evident audit
  - Stellar RPC, transaction assembly and submission
        |
        v
Exact-envelope signing backend
        |
        v
Fresnica SDK / Core
  - protected signer envelope
  - Account / Signer / Recovery Source separation
  - expected signer identity verification
  - exact XDR + network-passphrase signing
  - passphrase / System Auth security boundary
```

Fresnica remains the signer and protected-wallet security authority. Stellar Agent Wallet remains the Agent execution, policy, approval, audit and MCP authority.

This is composition, not source copying. The integration should consume pinned upstream crates or an accepted upstream extension point rather than importing a private fork of the full project into Fresnica.

## What Fresnica should reuse

The reviewed Stellar Agent Wallet already implements the components that a local Agent wallet service needs:

- an MCP stdio server with bounded framing and process hardening;
- one policy path shared by MCP and CLI behavior;
- typed value effects, per-transaction caps, rolling-window caps, rate limits and counterparty constraints;
- simulate-then-commit with a nonce bound to the exact envelope;
- operator approval cryptographically bound to the exact envelope hash;
- replay protection;
- fail-closed audit-writer acquisition before signing;
- hash-chained audit records;
- Stellar RPC account reads, transaction construction, submission and confirmation polling;
- existing protocol/tool work such as SEP-43, SEP-53, x402 and MPP.

Fresnica should not independently recreate those mechanisms.

## What remains Fresnica-owned

The following must not be replaced by the external project's current S-strkey-in-keyring custody path:

- the protected signer envelope;
- verified unlock-key derivation;
- separation of Account identity from Signer capability;
- signer identity validation before use;
- routine signing versus Reveal / Export privilege separation;
- Native SDK and System Auth integration;
- exact-envelope and network-passphrase signing semantics;
- future support for Fresnica external or hardware signer providers.

Fresnica must not decrypt a protected signer to an S-strkey and enroll that secret into the Stellar Agent Wallet keyring. That would duplicate secret material, create a second custody format and bypass the Fresnica signer lifecycle.

## The current integration blocker

The existing low-level Stellar Agent Wallet `Signer` trait is a **digest signer**:

```text
sign_tx_payload([u8; 32]) -> [u8; 64]
```

Its sanctioned `attach_signature` call site decodes the envelope, derives the SEP-23 transaction hash from the envelope and network passphrase, invokes the signer with only that hash, and appends the resulting signature.

Fresnica intentionally exposes a stronger signer request:

```text
TransactionSigningRequest
  - transaction_hash
  - exact transaction_xdr
  - network_passphrase
```

The protected SDK path signs from the exact XDR and network, verifies expected signer identity and returns the signed envelope.

A naive implementation of the external digest-only `Signer` trait would therefore reduce Fresnica to an arbitrary 32-byte signing oracle. That is prohibited even if the current caller computes the hash correctly.

The current MCP `stellar_pay_commit` implementation also directly constructs a keyring-backed signer. `WalletServer` has no injected signer backend. Therefore a safe Fresnica adapter cannot be added without first introducing a narrow upstream composition seam.

## Required upstream seam

Do not replace the existing low-level `Signer`; it remains useful for software and hardware implementations inside Stellar Agent Wallet.

Add an application-level exact-envelope backend at the point immediately before signing:

```rust
pub struct ClassicEnvelopeSigningRequest<'a> {
    pub source_account: &'a str,
    pub expected_signer_public_key: &'a str,
    pub unsigned_envelope_xdr: &'a str,
    pub network_passphrase: &'a str,
}

pub struct ClassicEnvelopeSigningResult {
    pub signed_envelope_xdr: String,
    pub signer_public_key: String,
    pub signer_kind: SubmissionSignerKind,
}

#[async_trait]
pub trait ClassicEnvelopeSigningBackend: Send + Sync {
    async fn sign_classic_envelope(
        &self,
        request: ClassicEnvelopeSigningRequest<'_>,
    ) -> Result<ClassicEnvelopeSigningResult, WalletError>;
}
```

The exact names are not normative. The required properties are:

1. the backend receives the exact unsigned envelope and network passphrase, never only a digest;
2. Account source and expected Signer identity are separate fields;
3. the backend returns a signed envelope plus non-secret signer attribution;
4. Stellar Agent Wallet verifies that the returned envelope preserves the original transaction and only adds valid expected signature material;
5. policy, approval, audit preflight, envelope divergence checks and nonce/replay checks remain before the backend call;
6. submission continues to receive already-signed XDR and never receives signer authority.

The default implementation should wrap the current keyring flow:

```text
KeyringClassicEnvelopeSigningBackend
  -> signer_from_keyring
  -> attach_signature
```

`WalletServer::new(profile)` should retain the default behavior. An additional constructor or builder may accept `Arc<dyn ClassicEnvelopeSigningBackend>` for embedders.

## Fresnica adapter

After the upstream seam exists, a small integration crate can implement it without adding an Agent stack to Fresnica:

```text
FresnicaClassicEnvelopeSigningBackend
  -> load configured protected signer reference
  -> obtain an already-authorized routine-signing credential from the trusted host
  -> call FresnicaSdk::sign_transaction_xdr
  -> verify expected signer identity
  -> return the signed envelope
```

The adapter must not expose to MCP or Agent tools:

- wallet passphrase;
- mnemonic or secret;
- Reveal / Export;
- raw `WalletUnlockKey`;
- arbitrary message or digest signing;
- arbitrary Process Binding operations.

The existing Fresnica Process Binding remains a privileged owner interface and is not the Agent integration protocol.

## First proof: one Testnet Payment

The first executable integration must be deliberately narrow:

- Classic TxV1 only;
- one direct-master Classic account;
- exactly one Payment operation;
- Testnet only;
- no pre-existing signatures;
- no Fee Bump;
- no delegated or multisig signer resolution;
- no arbitrary XDR tool;
- no Mainnet override.

Acceptance sequence:

1. a Stellar Agent Wallet V1 policy allows one bounded Testnet payment;
2. `stellar_pay` builds the exact envelope and mints its nonce;
3. `stellar_pay_commit` re-derives authoritative fields, re-evaluates policy, verifies approval when required, byte-compares the rebuilt envelope, acquires the audit writer and consumes the nonce;
4. the exact envelope and network passphrase reach the Fresnica backend;
5. Fresnica signs through its protected signer path without exporting an S-strkey or owner passphrase to MCP;
6. Stellar Agent Wallet verifies the returned signed envelope and submits it;
7. the transaction confirms on Testnet and the audit chain records the action;
8. mutation tests prove that changing destination, asset, amount, fee, source, network, envelope bytes or nonce is refused before signing.

A test fixture may use a known test passphrase to derive a credential inside the test process. That is not a production host-authorization design.

## Staged work

### Stage A — upstream extension design

- propose the exact-envelope signing backend to Soneso;
- keep the default keyring behavior unchanged;
- add a mock backend test proving `stellar_pay_commit` passes the exact nonce-bound envelope and network;
- add a returned-envelope validation test.

### Stage B — Fresnica adapter

- implement only the backend adapter;
- depend on pinned upstream crate versions or commits;
- do not copy the MCP server, policy engine, approval store, nonce crate or audit implementation;
- add cross-project fixtures for exact XDR/network/signer identity.

### Stage C — Testnet MCP acceptance

- compose a small host binary from the upstream MCP library and Fresnica backend;
- execute the one-payment flow;
- record exact source commits and dependency locks;
- run the relevant upstream and Fresnica regression suites.

### Stage D — expansion only from evidence

Consider more operation families, multisig, System Auth product integration or Mainnet only after the Payment proof and a fresh security review. Each new operation family requires typed policy semantics; a generic operation-type allowlist remains insufficient.

## Explicit non-goals

This decision does not authorize:

- a Fresnica-specific replacement MCP server;
- a new local HTTP daemon or remote wallet service;
- copying the Stellar Agent Wallet policy/approval/audit code into Fresnica;
- using Fresnica Process Binding as an Agent API;
- implementing the dormant Fresnica `AgentCapability` as a parallel production policy engine;
- exposing arbitrary hash signing to satisfy the external `Signer` trait;
- storing a second copy of Fresnica signing material in the external keyring;
- Mainnet autonomous signing while the upstream project structurally refuses it.

## Re-evaluation triggers

Revisit this decision only if:

- the upstream project stops maintaining the relevant crates or changes license incompatibly;
- the upstream policy/approval/audit model cannot preserve exact-envelope binding;
- an upstream exact-envelope signer seam is rejected and cannot be maintained as a small, reviewable adapter patch;
- a concrete product requires semantics that cannot be represented safely by the upstream policy engine;
- independent security review finds a material flaw that makes reuse riskier than a narrow replacement.

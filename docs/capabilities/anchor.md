# Anchor Capability

Maturity: **Normative** for the Classic common path described below.

Updated: 2026-08-26

## Purpose

The Anchor Capability defines wallet-facing Stellar anchor/SEP semantics below product UI.

Application Flows own deposit/withdraw/KYC presentation. The Capability owns protocol discovery, authentication meaning, protocol selection, normalized transfer/status/customer semantics and the safe handoff to ordinary transaction/payment capabilities.

## Asset and network identity

Anchor operations are scoped by:

```text
Stellar network + full issued-asset identity (CODE:GISSUER)
```

Code-only asset identity is not sufficient. Issued-asset code comparison is exact/case-sensitive; SEP-1 metadata lookup must not match `USD` to a distinct protocol-valid `usd` asset merely for convenience.

Discovery/session/customer/transaction state must not leak across network or anchor boundaries.

## Current normative Classic scope

The shared Classic `G...` account path covers:

- SEP-1 discovery;
- SEP-10 challenge authentication;
- SEP-24 interactive deposit/withdraw;
- SEP-6 deposit/withdraw fallback;
- anchor transaction status;
- reviewed withdrawal-payment handoff;
- common SEP-12 customer status and scalar/binary customer update handoff.

SEP-45 contract-account execution is deliberately outside this Classic normative path until its Soroban authorization semantics are specified separately. Official SEP/auth libraries supporting additional account families do not automatically widen Fresnica's current scope: unsupported `M...`/contract/delegated paths must fail explicitly and must not be demuxed/reinterpreted into the direct Classic `G...` path.

## SEP-1 discovery

A conforming implementation should discover and validate applicable anchor metadata from the asset issuer's `home_domain`, including the endpoints needed for supported flows such as:

- `TRANSFER_SERVER`;
- `TRANSFER_SERVER_SEP0024`;
- `WEB_AUTH_ENDPOINT`;
- `KYC_SERVER` where present;
- Classic `SIGNING_KEY`;
- contract-auth metadata when only reporting capability availability.

Discovery must distinguish "metadata exists" from "a complete executable authentication path exists".

Anchor protocol endpoints used for authenticated/sensitive flows must be HTTPS, have a real host and must not contain embedded username/password credentials. Redirect handling must preserve an equivalent security boundary: an initially valid HTTPS endpoint must not silently downgrade to HTTP or otherwise bypass endpoint-origin policy through automatic redirects. The Rust reference additionally rejects IP literals, local/private-name forms (`localhost`, `.local`, `.home.arpa`) and single-label hosts; `home_domain` cannot carry a port, while HTTPS protocol endpoints preserve an explicitly advertised transport port. Its Anchor transport uses bounded timeouts and response bodies rather than allowing indefinite or unbounded reads.

## Transfer protocol selection

For the current product policy:

1. prefer a usable SEP-24 flow when available;
2. fall back to SEP-6 when usable;
3. do not invent a Fresnica-specific transfer protocol when neither SEP path is valid.

When status lookup is ambiguous between protocols, the implementation must not silently guess if protocol identity affects endpoint/authentication semantics.

## Classic SEP-10 authentication

SEP-10 authentication is a two-phase integrity boundary:

```text
request challenge
  -> verify server-provided challenge completely
  -> expose verified challenge read-only to signing layer
  -> sign exact verified challenge through Fresnica SDK/Core
  -> verify signed result is still the same challenge
  -> exchange for token
```

A conforming implementation must reject before token exchange when:

- the challenge targets the wrong account/domain;
- structural/time/server-signature validation fails;
- the signed transaction body is substituted;
- the required server signature is missing/invalid;
- the client signature is missing/invalid.

A product must never ask a protected signer to sign arbitrary server-provided XDR before SEP-10 verification.

Classic `G...` authentication does not imply "the master key alone always signs". Where the account's current ledger signer/threshold configuration requires multisig authorization, the verified challenge must flow through Ledger Authorization + Signing Coordination. The Rust reference now coordinates enough local software Ed25519 signers to meet the current medium threshold; Hash-X, signed-payload and external/provider signer conditions remain unsupported and fail explicitly.

Authorization tokens are session material, not wallet truth. They must not be logged, printed as normal output, persisted as ordinary wallet state or passed through command-line arguments.

## SEP-24

The common semantic result of SEP-24 initiation includes the anchor transaction identity and interactive flow information required by the product.

The interactive webview/browser UX is Flow/platform-owned.

SEP-24 status/authentication behavior must follow the SEP rather than UI assumptions.

## SEP-6

SEP-6 initiation/status must respect the anchor's advertised `/info` authentication requirements and field semantics.

SEP-6 transport fields are protocol mechanics; product screens may collect equivalent information in a platform-appropriate way.

## Transaction status

Status lookup is read-only by default.

The Capability should preserve the anchor transaction identity, kind, status and protocol fields needed by product Flows. A platform may retain the raw protocol object for diagnostics, but ordinary UI should consume normalized semantic state rather than branch on arbitrary JSON strings where a stable state is defined.

## Withdrawal payment handoff

An anchor status response does not itself authorize a blockchain payment.

When a withdrawal reaches the protocol state requiring user transfer, the Anchor Capability derives the required payment intent and hands it to the ordinary Payment/Transaction path.

The product must still present the real payment review and obtain normal signing authorization.

Current Classic handoff preserves:

- withdrawal destination account;
- `amount_in`;
- protocol memo type/value;
- text, unsigned ID and 32-byte hash memo semantics.

Anchor code must not flatten a non-text memo into text or bypass Payment preflight/signing rules.

## SEP-12 customer information

Common SEP-12 support includes:

- authenticated customer status lookup;
- customer ID/type/transaction context where applicable;
- required/provided field state;
- scalar SEP-9 value updates;
- binary multipart field uploads.

Sensitive customer values are private application input and must not be deliberately placed into logs, analytics or process-visible command arguments.

Binary fields are protocol payloads, not durable wallet identity.

Nested structured customer values plus the optional `/customer/files` file-ID workflow remain outside the current common contract until a concrete anchor requires them.

## Errors

Flows should be able to distinguish at least:

- discovery/capability unavailable;
- authentication required/unsupported account auth family;
- invalid authentication challenge;
- user/signing authorization failure;
- protocol/request validation failure;
- KYC/customer information required;
- transfer pending/processing/rejected/complete state;
- network/anchor transport failure.

Do not expose JWTs or submitted KYC values merely to make an error string more descriptive.

## Security ownership

### Anchor Capability implementation

Owns SEP protocol validation and transport semantics.

### Fresnica SDK/Core

Owns transaction/XDR hashing, Ed25519 signing/verification, signer identity verification and protected software signing.

### Application Flow

Owns forms, KYC screens, browser/webview behavior, confirmation and product-facing status presentation.

## Reference extension: semantic next actions (non-normative)

RefPython provides a useful application-facing projection above raw SEP-6/SEP-24 responses:

- [`reference/python/fresnica/anchor_transfer_service.py`](../../reference/python/fresnica/anchor_transfer_service.py)
- [`reference/python/tests/test_anchor_transfer_service.py`](../../reference/python/tests/test_anchor_transfer_service.py)

Instead of forcing a Flow to branch directly on protocol JSON, the reference translates protocol state into semantic next-action categories such as:

```text
NeedFields
OpenUrl
KycRequired
DepositInstructions
WithdrawalPayment
```

The Python type/class names are not normative. The candidate cross-platform idea is that protocol mechanics should be normalized into **what the wallet/user must do next**, while still retaining enough protocol identity/raw diagnostics for recovery and debugging.

This is especially useful for Mobile because SEP-24 browser handoff, SEP-6 required fields/KYC and withdrawal-payment construction are different UX actions even when all originate from one Anchor Capability. Independent platform evidence should determine whether a shared next-action/result model is stable enough for promotion.

## Reference implementation status

The current Rust reference implementation (`clients/rust-client::anchor` and `anchor_protocol`) implements most of the Classic scope above. The Rust CLI is a presentation/orchestration consumer and keeps authentication tokens in zeroizing in-memory values.

The Rust and RefPython references require exact-case SEP-1 `code + issuer` identity and reject automatic HTTP redirects for Anchor protocol requests, preserving the endpoint security boundary rather than following a redirected target implicitly. Rust Classic SEP-10 now builds a Ledger Authorization plan from current Horizon state and uses Signing Coordination to collect only the needed local software Ed25519 signatures. Activated accounts use the medium threshold and still require at least one actual client signature when that threshold is zero; unactivated accounts use the SEP-10 master-key proof; the anchor server key is explicitly excluded from client weight. RefPython still rejects an attached local signer whose key differs from the Classic account identity. Hash-X, signed-payload and external/provider signer collection remain deferred, so this is not general delegated-signing support.

Current deferred areas:

- SEP-45 contract-account authentication execution;
- uncommon nested SEP-12 + `/customer/files` workflow;
- concrete-anchor compatibility fixes discovered through real integration.

These are demand-driven extensions, not reasons to weaken the existing Classic contract.

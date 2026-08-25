# Anchor Authentication Boundary

Updated: 2026-08-25

This document records the current Rust reference-client authentication boundary for Stellar anchors.

## Current state

The Rust CLI `anchor discover CODE:GISSUER` now discovers and validates:

- SEP-1 `stellar.toml` from the issuer `home_domain`;
- SEP-6 `TRANSFER_SERVER` and `/info` capability metadata;
- SEP-24 `TRANSFER_SERVER_SEP0024` and `/info` capability metadata;
- SEP-10 `WEB_AUTH_ENDPOINT` plus a Classic `G...` `SIGNING_KEY`;
- SEP-45 `WEB_AUTH_FOR_CONTRACTS_ENDPOINT`, Contract `C...` `WEB_AUTH_CONTRACT_ID`, and `SIGNING_KEY`;
- whether authenticated SEP-6/SEP-24 use has at least one complete SEP-10 or SEP-45 metadata path.

The Rust CLI also implements the Classic-account SEP-10 session path:

- `anchor auth CODE:GISSUER [--wallet NAME]` requests and verifies a challenge before signing it through the Fresnica SDK/Core boundary, exchanges it for a JWT, and immediately discards that JWT after the diagnostic command completes;
- `anchor deposit|withdraw CODE:GISSUER ...` prefers a usable SEP-24 flow and falls back to SEP-6;
- SEP-24 interactive requests use `multipart/form-data` and a fresh in-memory SEP-10 token;
- SEP-6 only authenticates when the selected asset's `/info.authentication_required` is true;
- JWT values are held only in zeroizing in-memory strings and are never printed, persisted, logged, or passed through CLI arguments.

Transfer execution in the Rust CLI is currently Classic `G...` / SEP-10 only. SEP-45 metadata is discovered but contract-account authentication execution is intentionally still separate.

## Authentication model

Account identity selects the authentication family:

- Classic `G...` account: SEP-10.
- Contract `C...` account: SEP-45.

SEP-10 and SEP-45 are parallel authorization mechanisms. SEP-45 does not replace SEP-10.

## Layer ownership

The implementation preserves these boundaries:

### Anchor service / protocol layer

Owns protocol validation and transport semantics, including:

- requesting an authentication challenge;
- validating challenge structure, target account, domain and time constraints;
- choosing SEP-10 versus SEP-45 from account identity and discovered capabilities;
- exchanging a verified signed challenge for an authorization token;
- keeping the token/session scoped to the anchor and network.

### Rust Core

Owns low-level cryptographic/XDR authority, including:

- transaction XDR parsing and hashing;
- Ed25519 signature verification;
- signer identity verification;
- transaction signature application.

Core must not absorb HTTP anchor transport or SEP-specific product flow.

### Fresnica SDK

Owns the safe application-facing signing boundary. After a challenge has been verified, protected software signing must go through the SDK/Core signing path; the CLI must not derive or manipulate private keys itself.

### CLI / product UI

Owns only command/UX orchestration and user authorization prompts. It must not sign arbitrary server-provided XDR before protocol verification.

## Next implementation slice

The current transfer command stops at the protocol-defined next action:

- SEP-24 returns a validated interactive URL (and transaction id when supplied);
- SEP-6 returns the anchor's structured deposit/withdraw instructions, including KYC-required responses;
- Fresnica does **not** automatically send the Stellar withdrawal payment from an anchor response in this slice.

The next withdrawal slice should translate verified SEP-6 withdrawal instructions into the existing reviewed payment path, require normal human transaction review/confirmation, and add anchor transaction-status lookup. It must not introduce a second signing path.

SEP-45 remains a separate contract-account path because its Soroban authorization-entry verification/signing semantics differ from Classic SEP-10 transaction signing.

## Current blockers / non-blockers

- Classic SEP-10 verifier/session and initial SEP-24/SEP-6 transfer initiation are implemented in the Rust reference client.
- SEP-45 metadata discovery is complete; SEP-45 execution still requires a dedicated contract-auth provider/verification path.
- Ledger transport remains independent and must not be forced through a lossy XDR v28/v27 conversion merely to close a checklist item.

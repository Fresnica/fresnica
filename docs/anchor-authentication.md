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

Discovery is metadata/capability work only. It does **not** sign an authentication challenge, obtain a JWT, or execute an authenticated deposit/withdraw request.

## Authentication model

Account identity selects the authentication family:

- Classic `G...` account: SEP-10.
- Contract `C...` account: SEP-45.

SEP-10 and SEP-45 are parallel authorization mechanisms. SEP-45 does not replace SEP-10.

## Layer ownership

The next implementation must preserve these boundaries:

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

For SEP-10, add a verifier before any signing API is invoked. At minimum it must reject a challenge unless all required SEP-10 properties are valid, including:

- expected server signing key signature;
- zero sequence number;
- valid time bounds;
- expected client account;
- first ManageData operation and `<home_domain> auth` key;
- expected `web_auth_domain` semantics when present;
- allowed operation/source-account constraints.

Only after verification may Fresnica sign the challenge through the existing SDK signing boundary and POST it for a token.

SEP-45 should remain a separate contract-account path because its Soroban authorization-entry verification/signing semantics are different from classic SEP-10 transaction signing.

## Current blockers / non-blockers

- SEP-10 verifier/session work is unblocked and is the next anchor slice.
- SEP-45 metadata discovery is complete; SEP-45 execution still requires a dedicated contract-auth provider/verification path.
- Ledger transport remains independent and must not be forced through a lossy XDR v28/v27 conversion merely to close a checklist item.

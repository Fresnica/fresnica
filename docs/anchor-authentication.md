# Anchor Authentication Boundary

Updated: 2026-08-26

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
- SEP-24 interactive requests use `multipart/form-data`, require the anchor's transaction `id`, and use a fresh in-memory SEP-10 token;
- `anchor status CODE:GISSUER ID ...` queries the protocol `/transaction` endpoint; when SEP-24 and SEP-6 are both plausible the caller must provide `--protocol` rather than letting Fresnica guess;
- SEP-24 status always authenticates; SEP-6 status follows `/info.transaction.authentication_required`;
- `anchor status ... --pay` is an explicit write transition only when the transaction is a withdrawal in `pending_user_transfer_start`; it consumes `withdraw_anchor_account`, `amount_in`, and `withdraw_memo[_type]`, then hands the payment to the existing reviewed transaction path;
- withdrawal memos support the protocol-defined `text`, `id`, and base64 `hash` forms without flattening them to text;
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

## Transaction status and payment boundary

The Rust reference client treats status lookup and payment as separate actions:

- status lookup is read-only by default and returns the anchor's full transaction object in `--json` mode;
- `--pay` cannot be combined with `--json` and never bypasses the normal transaction review; `-y/--yes` has the same explicit confirmation-bypass semantics as the existing `send` command;
- Fresnica refuses to pay unless the anchor transaction is a withdrawal whose status is exactly `pending_user_transfer_start`;
- the payment destination must be a Classic `G...` account, the amount is revalidated by the existing payment path, and the memo is converted to its real XDR memo type before envelope construction;
- signing and submission still go through the Fresnica SDK/Core path and the existing pending-transaction safety gate.

The Rust reference client now has the first SEP-12 customer-information handoff for `pending_customer_info_update` and related KYC states. `anchor customer CODE:GISSUER` authenticates with a fresh in-memory SEP-10 token, prefers `KYC_SERVER` and falls back to `TRANSFER_SERVER`, returns typed customer/required/provided-field state, and can submit scalar SEP-9 values plus binary multipart fields. Sensitive customer values are read from `--input PATH` or `--input -` (stdin), not command-line `--field` arguments, so KYC data is not deliberately copied into shell history or the process list.

SEP-12 nested structured values combined with the optional `/customer/files` file-id workflow remain a follow-up for anchors that require that less-common shape. SEP-45 remains a separate contract-account path because its Soroban authorization-entry verification/signing semantics differ from Classic SEP-10 transaction signing.

## Current blockers / non-blockers

- Classic SEP-10 verifier/session, SEP-24/SEP-6 initiation, transaction status lookup, and reviewed Classic withdrawal payment handoff are implemented in the Rust reference client.
- SEP-12 customer status plus common scalar/binary customer updates are wired through the shared Rust client service; nested structured values plus the optional `/customer/files` workflow remain follow-up work.
- SEP-45 metadata discovery is complete; SEP-45 execution still requires a dedicated contract-auth provider/verification path.
- Ledger transport remains independent and must not be forced through a lossy XDR v28/v27 conversion merely to close a checklist item.

# Payment Capability

Maturity: **Normative**

## Purpose

The Payment Capability prepares the semantic meaning and review of a Stellar payment before a product asks the user to authorize signing.

UI form structure and confirmation presentation belong to the Send/Anchor Flow, not to this capability.

## Semantic request

A payment request contains the semantic equivalents of:

- source account context;
- destination;
- amount;
- full asset identity (`XLM` or `CODE:GISSUER`);
- optional memo.

Wallet/account selection UI and repository lookup belong to the Flow/application layer. The Capability receives the resolved source account context.

Implementations may additionally accept a contact/destination alias through the Contacts capability.

## Numeric and asset rules

- amount must be positive and representable at Stellar seven-decimal precision;
- issued-asset identity must include both code and issuer;
- asset codes follow Stellar Classic issued-asset constraints;
- binary floating point must not determine transaction amounts.

## Destination and operation selection

The current Normative payment scope uses Classic `G...` destinations. Muxed-account, contract-payment and path-payment semantics should be added deliberately rather than inferred from this contract. A platform SDK accepting `M...` or Path Payment does not expand this contract; unsupported forms must fail explicitly and must not be silently demuxed/reinterpreted into the current `G...` path.

For Classic payments:

- if the destination account exists, use `Payment` semantics;
- if the destination does not exist, only native XLM may create it;
- a missing destination receiving XLM uses `CreateAccount` semantics;
- the create amount must satisfy the current minimum starting balance requirement;
- issued assets require an existing destination account and applicable trustline semantics.

For issued-asset Payment in the current supported protocol semantics:

- an ordinary source trustline must be fully authorized for ordinary sending; `AUTHORIZED_TO_MAINTAIN_LIABILITIES` is not sufficient for a new Payment;
- an ordinary destination must have the exact receiving trustline, full authorization for ordinary receipt and sufficient receiving capacity;
- the asset issuer itself is a protocol special case and does not need a self-trustline; sending its own asset is issuance and receiving its own asset is redemption;
- an issuer account having been removed does not by itself invalidate an already-existing issued asset/trustline for Payment on current protocol versions; do not add an `issuer must still exist` preflight that Stellar Core no longer requires.

A review must expose which operation will actually be submitted.

## Availability preflight

Preparation must use the Balance / Availability semantics and current ledger parameters to reject insufficient:

- source asset balance after selling liabilities;
- native reserve capacity;
- transaction fee capacity.

For issued assets, native fee availability remains required. Source-issuer issuance must use the issuer special case above rather than ordinary holder balance lookup.

## Memo-required destinations (SEP-29)

For the current non-muxed Classic destination scope, a destination account advertising `config.memo_required=1` must not receive a memo-less payment through an ordinary Fresnica Send/Payment flow. SEP-29 is a client-side safety requirement: the network may accept the transaction, so the wallet must detect the flag and require an appropriate transaction memo before signing/submission.

This check belongs to preparation/security semantics, not only to a UI warning.

## Memo semantics

Shared payment semantics support no memo and the Stellar memo forms required by current product protocols, including:

- text;
- unsigned 64-bit ID;
- 32-byte hash.

A normal text memo is limited by Stellar XDR, not by UI character assumptions. Protocol adapters such as Anchor must preserve the true memo type rather than flattening every memo to text. `MEMO_RETURN` is outside the current shared Payment/Anchor scope; it must not be silently treated as ordinary hash memo semantics.

## Prepared review

The prepared review must represent the exact semantic transaction being authorized, including at least:

- operation (`Payment` or `CreateAccount`);
- source and destination;
- amount and full asset identity;
- network;
- fee;
- memo when present;
- resolved contact label when the product uses one.

The prepared transaction/envelope itself remains opaque to ordinary UI code unless a lower-level review/debug surface explicitly needs it.

## Signing and submission

Preparation must reject a source with no applicable signing capability before a write is presented as executable.

Final authorization/signing follows the [Signing Coordination](signing-coordination.md) and [Transaction](transaction.md) contracts. A Flow owns the confirmation point; it must not rebuild a different transaction after review.

## Errors

Stable product-level categories should cover invalid input/asset/destination, insufficient balance/reserve/fee, destination/trustline incompatibility, watch-only/no signer, authorization/signing failure and submission result.

## Reference implementation

The current Rust reference implementation is `reference/rust-client::payment` and is consumed by both CLI and TUI.

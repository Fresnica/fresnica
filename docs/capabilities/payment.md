# Payment Capability

Maturity: **Normative**

## Purpose

The Payment Capability prepares the semantic meaning and review of a Stellar payment before a product asks the user to authorize signing.

UI form structure and confirmation presentation belong to the Send/Anchor Flow, not to this capability.

## Semantic request

A payment request contains the semantic equivalents of:

- source account/wallet selection;
- destination;
- amount;
- full asset identity (`XLM` or `CODE:GISSUER`);
- optional memo.

Implementations may additionally accept a contact/destination alias through the Contacts capability.

## Numeric and asset rules

- amount must be positive and representable at Stellar seven-decimal precision;
- issued-asset identity must include both code and issuer;
- asset codes follow Stellar Classic issued-asset constraints;
- binary floating point must not determine transaction amounts.

## Destination and operation selection

The current Normative payment scope uses Classic `G...` destinations. Muxed-account, contract-payment and path-payment semantics should be added deliberately rather than inferred from this contract.

For Classic payments:

- if the destination account exists, use `Payment` semantics;
- if the destination does not exist, only native XLM may create it;
- a missing destination receiving XLM uses `CreateAccount` semantics;
- the create amount must satisfy the current minimum starting balance requirement;
- issued assets require an existing destination account and applicable trustline semantics.

A review must expose which operation will actually be submitted.

## Availability preflight

Preparation must use the Balance / Availability semantics and current ledger parameters to reject insufficient:

- source asset balance after selling liabilities;
- native reserve capacity;
- transaction fee capacity.

For issued assets, native fee availability remains required.

## Memo semantics

Shared payment semantics support no memo and the Stellar memo forms required by current product protocols, including:

- text;
- unsigned 64-bit ID;
- 32-byte hash.

A normal text memo is limited by Stellar XDR, not by UI character assumptions. Protocol adapters such as Anchor must preserve the true memo type rather than flattening every memo to text.

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

The current Rust reference implementation is `clients/rust-client::payment` and is consumed by both CLI and TUI.

# Dapp Interaction Capability

Maturity: **Defined**

## Purpose

Dapp Interaction is the shared capability name for receiving, reviewing, authorizing and responding to an external application's wallet request.

The capability deliberately standardizes **the name, purpose and security boundary first**, not one transport API.

## Current semantic boundary

A Dapp implementation may support requests involving:

- account identity/connection;
- transaction review/signing;
- transaction submission;
- SEP-53 message signing for explicit dapp challenges;
- other wallet authorization operations explicitly supported by Fresnica.

Every request must reuse existing Account, Transaction, Signer and Signing Coordination semantics. Dapp transport must not create an alternate signing/security model.

Remote application names, icons, descriptions, requested amounts and other display metadata are **untrusted metadata**. When a request asks the wallet to sign a Stellar transaction, the authoritative review must be derived from the exact transaction/envelope semantics that will be signed, not from the remote application's claim about what that transaction does. Remote metadata may supplement that review but cannot replace or contradict it.

If Fresnica cannot fully understand the transaction/authorization semantics required for its normal review policy, the ordinary Dapp approval path must fail closed. "The SDK can parse this XDR" is not sufficient evidence that the wallet can safely explain/authorize it. A future explicitly designed expert/raw-signing Flow may choose another policy, but ordinary Dapp approval must not silently degrade into blind signing.

For a message-signing request, the authoritative review object is the exact message text/bytes that enter the SEP-53 domain. The Mobile React Native bridge uses JavaScript `String` -> UTF-8 bytes without normalization; Core/SDK retain raw-byte support for non-framework consumers. The transport may encode the resulting 64-byte Ed25519 signature for its wire protocol, but must not reinterpret the message before signing.

SEP-53 deliberately does **not** carry network passphrase, origin, nonce, expiry, or account scope. An authentication challenge must therefore include the security-significant fields in the reviewed message and/or bind them independently to the authenticated dapp session. Replay prevention, challenge expiry, peer/origin validation, bounded/displayable challenge policy and SEP-43 session/network selection remain Dapp/product responsibilities above Core.

A valid SEP-53 signature proves control of the selected Ed25519 signer key, **not ledger authority over an arbitrary Stellar account**. In multisig or disabled-master-key accounts, a signer key can differ from the account ID and its weight may be insufficient for account control. Dapp product semantics must preserve `Account identity != Signer capability`; they must not relabel a signer-key proof as full account ownership without separate ledger-policy evidence.

## Platform-specific mechanisms

Examples include:

- Mobile WalletConnect-style transport;
- deep links;
- in-app browser bridges;
- Web extension/browser bridges;
- Desktop IPC/browser integration.

Transport/session discovery, framework lifecycle and UI are platform-owned.

## Security requirements

A Dapp request must not:

- receive raw private software signer material;
- bypass transaction review/confirmation policy;
- substitute remote application-provided descriptions for review derived from the exact transaction being authorized;
- sign arbitrary bytes through the Stellar transaction-signing path or generic raw-hash signing;
- normalize, rewrite, or decorate a reviewed SEP-53 challenge after user review;
- treat a remote application's claimed account/signer identity as trusted without local validation;
- bypass the stronger Reveal/Export boundary.

A Dapp session/permission must also be bound to the actual remote peer/origin plus relevant network/account scope, not only a remote self-reported name/icon or a global `connected=true` flag. Switching origin, network or account must not inherit an unrelated signing permission without explicit policy.

## Implementation evidence status

Fresnica now has one concrete implementation requirement from Mobile: SEP-53 challenge signing through the Native SDK / React Native adapter. It still does not have enough transport/session evidence to define a universal shared request/session/result model. This is intentional: Mobile or Web should not wait for a speculative universal API before building the first useful implementation.

The first concrete implementation should contribute evidence back to this document, including:

- request classes it actually needs;
- session/connection lifecycle that proved product-significant;
- review/result/error semantics that are independent of transport;
- which parts are WalletConnect/browser/deep-link mechanics only;
- regression tests or fixtures that demonstrate security boundaries.

A separate implementation repository such as `fresnica-mobile` may submit those findings as a documentation PR without moving its source code into this repository.

## Promotion criteria

When real Mobile/Web/Desktop implementations reveal a stable cross-platform request/review/result/session model, propose those semantics for promotion to `Normative`. Until then, do not freeze WalletConnect or any other single transport as the universal Fresnica Dapp API.

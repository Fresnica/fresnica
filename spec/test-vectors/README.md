# Fresnica cross-language test vectors

These files define language-neutral behavior that future Fresnica cores and clients must agree on. They are semantic fixtures, not serialized Python objects and not a public wire protocol.

Current sets:

- `wallet-v1.json`: deterministic SEP-0005 mnemonic/passphrase/account-index derivation to Stellar public keys.
- `protection-v1.json`: password and system-key wallet-secret encryption compatibility.
- `transaction-signing-v1.json`: Classic transaction hashing, Ed25519 signature, signature hint, and signed-envelope compatibility.
- `sdex-v1.json`: pair-relative SDEX intent, offer projection, fill projection, and compression behavior.
- `asset-identity-v1.json`: exact Classic native/issued asset identity, including case-sensitive issued codes and `xlm:G...` constructor-normalization hazards.
- `smart-account-auth-v1.json`: real Protocol 27 Testnet smart-account authorization captured after a confirmed WebAuthn/passkey transfer and accepted by the provider fixture verifier.

The smart-account vector is captured public authorization material from Testnet, not a synthetic passkey fixture. It contains the confirmed transaction identity, host-function XDR and signed Soroban auth XDR required to replay the provider conformance checks; it does not contain a passkey private key or Fresnica software-wallet secret.

## Versioning

Existing vector meaning must not be changed silently. If a later implementation intentionally changes a normative behavior, add a new version and document the compatibility decision.

## Wallet derivation

`wallet-v1.json` uses public SEP-0005 / stellar-sdk reference cases. Fresnica stores only the mnemonic/passphrase/index inputs and expected **public key** outputs; upstream test secret seeds are intentionally not copied into this repository.

The derivation contract is Stellar SEP-0005, including the account index path and BIP39 passphrase. A future Rust Core must produce the same public key for every vector before it can be considered wallet-compatible.

## Secret protection

`protection-v1.json` fixes public test-only payloads, salt, nonce, password, and wrapping-key bytes so implementations can prove they agree on the existing encrypted-wallet format. The values are not wallet credentials and must never be reused as production key material.

Password protection uses Scrypt with `N=32768`, `r=8`, `p=1` to derive a 32-byte key, followed by AES-256-GCM with AAD `fresnica-wallet-secret-v1`. System protection uses an externally stored 32-byte wrapping key with AES-256-GCM and AAD `fresnica-wallet-secret-key-v1`.

The vector freezes compatibility with existing Fresnica records; it does not claim those parameters are immutable for all future wallet formats. A future format upgrade must use a new explicit version rather than silently reinterpreting version 1 ciphertext.

## Classic transaction signing

`transaction-signing-v1.json` uses a public RFC 8032 Ed25519 test key and a minimal Classic V1 Stellar transaction envelope on testnet. The test secret is intentionally public test material and must never be used as a wallet credential.

Implementations must agree on the unsigned envelope XDR, network-specific transaction hash, raw Ed25519 signature, four-byte Stellar signature hint, and final decorated-signature envelope XDR. The transaction hash is the signing payload; raw XDR and network passphrase are review/context inputs for external signers rather than arbitrary bytes to sign.

This vector deliberately covers Classic transaction signing only. SEP-53 arbitrary-message signing and Soroban contract authorization require separate domain-specific vectors.

## Classic asset identity

`asset-identity-v1.json` fixes native `XLM` separately from issued `CODE:GISSUER` identity. Issued code bytes and case are semantic: `USD:G...` and `usd:G...` are distinct, and `xlm:G...` remains an issued asset rather than native XLM. Platform SDK convenience constructors that normalize XLM spellings must use an exact construction path or fail explicitly instead of changing these vectors.

## Numeric rules

- Asset amounts and human prices are JSON strings containing base-10 decimals. Implementations must not pass them through binary floating point.
- Exact Stellar prices are represented as integer fractions: `{ "n": ..., "d": ... }`.
- User-facing projected values follow Stellar's seven-decimal precision and Fresnica's current half-up projection rule.
- Tests should compare decimal values numerically; insignificant trailing zeroes are not semantic.

## Market and offer semantics

A `pair` is always user-facing `BASE / COUNTER`.

- `amount` always means BASE units.
- `price` always means COUNTER units per one BASE unit.
- SELL encodes `ManageSellOffer(selling=BASE, buying=COUNTER, amount=BASE amount, price=COUNTER/BASE)`.
- BUY encodes `ManageBuyOffer(selling=COUNTER, buying=BASE, buyAmount=BASE amount, price=COUNTER/BASE)`.

A ledger `OpenOffer` is canonical chain state, not stored user intent. When canonical selling/buying are reversed relative to the selected pair, Fresnica projects it as a BUY by using the exact reciprocal price fraction. After partial fills, the displayed remaining BUY amount is a projection from canonical remaining selling amount; implementations must not treat it as a recoverable original order intent.

## Account trade aggregation

Account trades are compressed only when consecutive records can be proven to be fills from the same user offer at the same exact price fraction.

The normative merge identity is:

- market pair
- side
- exact price fraction
- identified user offer ID

Missing user offer IDs never merge. Equal records separated by another segment never merge across that gap. This deliberately avoids grouping unrelated AMM/path-payment activity merely because pair, side, and decimal price happen to match.

## What is deliberately not normative

Implementation-private values such as Python's `segment_key`, cache keys, database schemas, UI strings, class names, and storage layout are excluded. Rust, Python, and mobile implementations may organize those differently as long as they produce the same semantic results in the vectors.

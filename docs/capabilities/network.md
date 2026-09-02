# Network / Gateway Capability

Maturity: **Defined**

## Purpose

Network / Gateway is the shared capability name for accessing Stellar network state, submitting transactions and transporting Stellar ecosystem protocols.

It is Defined rather than Normative because Horizon/RPC/Portfolio/provider strategy is still evolving and may differ by product.

## Identity rule

Chain-derived wallet state is always network-scoped:

```text
Account context = Stellar network + account identity
```

The same Stellar address string may exist on more than one network. Implementations must not leak balances, liabilities, history, offers, transactions, pending-write guards, anchor discovery/session state or caches across network boundaries.

The exact network passphrase is security-significant because it participates in Stellar transaction hashing/signing. A product-visible network name is an alias/configuration label; it must resolve to the correct cryptographic network context.

## Provider transition rationale

The Rust reference uses Horizon for curated Classic account/balance/offer/trade/history resources, but this is transitional rather than a shared contract. It now also uses the official Protocol-28 `stellar-rpc-client` for Soroban network verification, account sequence/fee lookup, simulation, submission and status reconciliation. The resulting hybrid boundary is intentional: Soroban state/simulation/submission is RPC-backed while Classic signer/threshold discovery remains Horizon-backed until equivalent semantics are proven. Stellar's current API guidance marks Horizon as nearing end-of-life and recommends RPC for real-time access; the official Rust SDK set provides XDR/StrKey/RPC components but no SDF-maintained Horizon client. Fresnica therefore will not build a general Rust Horizon SDK.

```text
Application Capabilities -> Fresnica Network / Gateway -> Horizon (current) / RPC / Portfolio / history provider
```

`HorizonGateway` names the current Classic provider adapter; `RpcGateway` is the first-class Soroban RPC adapter. Provider JSON should terminate at the gateway/normalization boundary; write-policy code should consume typed semantic views as they are justified. Reuse official Stellar protocol types such as `stellar_xdr::Asset`. Migrate endpoint families to RPC/Portfolio only when equivalent semantics and uncertain-submission behavior are preserved. Do not build a lowest-common-denominator blockchain ORM.

Planned stages: isolate Horizon calls; normalize write-critical views; use RPC for covered Soroban real-time state/submission; add Portfolio/account data when suitable; retire Horizon endpoint families independently with conformance evidence.

## Stable responsibilities

Other Capabilities should be able to request semantic network operations such as:

- load current account/ledger state;
- test account existence;
- fetch operations/offers/trades/market data;
- obtain current fee/reserve parameters;
- submit an exact signed transaction;
- query transaction status;
- perform protocol HTTP transport required by Anchor or other capabilities.

The result should be normalized before ordinary Flows depend on transport-specific response shapes.

## Reference Semantics: Python and Rust implementations

Current reference implementations provide useful evidence for the shared network boundary:

- [`reference/python/fresnica/network.py`](../../reference/python/fresnica/network.py)
- [`reference/python/tests/test_cli_network.py`](../../reference/python/tests/test_cli_network.py)
- [`clients/rust-client/src/rpc_gateway.rs`](../../clients/rust-client/src/rpc_gateway.rs)
- [`clients/rust-client/src/soroban.rs`](../../clients/rust-client/src/soroban.rs)
- [`clients/rust-client/src/transaction.rs`](../../clients/rust-client/src/transaction.rs)
- [`clients/rust-client/src/service.rs`](../../clients/rust-client/src/service.rs)
- [`clients/rust-client/src/anchor_protocol.rs`](../../clients/rust-client/src/anchor_protocol.rs)

### 1. Network identity and provider endpoint are separate concerns

The Python reference stores a network profile containing both the Stellar network passphrase and a Horizon endpoint. The Rust reference similarly resolves known network names to passphrase/provider configuration.

The candidate common semantic is the separation itself:

```text
semantic / cryptographic network identity
        !=
current provider endpoint
```

A product may replace Horizon with RPC or another provider without changing which Stellar network a transaction belongs to. Conversely, a configured provider URL/name is not proof that the endpoint actually serves the intended network. When the provider exposes enough network identity/passphrase information to detect a mismatch, the application must fail closed before signing/protocol actions continue rather than sign for one network and treat submission to another as an ordinary transport error.

### 2. Durable and cached state is network-scoped

Reference tests deliberately reject wallet/network mismatches and keep caches separated by network. This is already a strong cross-capability invariant and should remain true regardless of provider implementation.

### 3. Network context is checked at security-sensitive protocol boundaries

The Rust Anchor SEP-10 implementation verifies that server-declared network context, when present, matches the local network configuration before accepting the challenge flow. Transaction hashing/signing likewise uses the selected network passphrase.

This reinforces that network choice is not merely a display setting.

### 4. Submission transport does not decide final transaction truth by timeout alone

A timeout or connection failure after submission may leave final chain outcome uncertain. The Transaction Capability owns the semantic distinction between deterministic rejection and uncertain submission; Network/Gateway must preserve enough information for reconciliation rather than collapsing both into one generic failure.

### 5. Provider families may migrate independently

The Rust Soroban reference proves that one endpoint family can move to RPC without forcing an all-at-once Classic migration. `RpcGateway` verifies the RPC network passphrase and owns Soroban simulation/submission/status transport, while final Classic account signer/threshold authorization still consumes the established Horizon-backed semantic plan. This hybrid is a staged implementation boundary, not a new shared requirement that products use both providers.

## Candidate semantics for promotion

1. Separate cryptographic network identity from provider endpoint selection.
2. Scope all chain-derived durable/cache state by network + domain identity.
3. Reject known network mismatches before signing/protocol actions continue.
4. Normalize provider-specific transport results before Flows consume them.
5. Preserve uncertain-submission semantics for later transaction reconciliation.
6. Allow provider endpoint families to migrate independently when their security and reconciliation semantics remain intact.

## Implementation freedom

The shared contract does not mandate:

- Horizon vs RPC vs Portfolio APIs;
- one HTTP client;
- one retry/backoff policy;
- one cache layout;
- one proxy/provider;
- one endpoint configuration format;
- literal `mainnet` / `testnet` names as the only possible profile identifiers;
- the current public Horizon URLs.

Typical products support mainnet and testnet. Custom endpoints/networks remain product policy unless promoted into a stronger shared contract.

## Promotion criteria

Promote narrower gateway semantics to Normative only when multiple platform implementations need to rely on the same normalized request/result model. Do not standardize a Horizon client wrapper merely because a reference implementation currently uses one.

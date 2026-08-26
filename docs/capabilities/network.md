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

## Implementation freedom

The shared contract does not mandate:

- Horizon vs RPC vs Portfolio APIs;
- one HTTP client;
- one retry/backoff policy;
- one cache layout;
- one proxy/provider;
- one endpoint configuration format.

Typical products support mainnet and testnet. Custom endpoints/networks remain product policy unless promoted into a stronger shared contract.

## Submission semantics

Network transport must preserve the Transaction Capability distinction between deterministic rejection and uncertain submission. A timeout/connection failure after sending bytes is not automatically proof that the transaction was rejected.

## Evolution

Promote narrower gateway semantics to Normative only when multiple platform implementations need to rely on the same result model. Do not standardize a Horizon client wrapper merely because the Rust reference currently uses one.

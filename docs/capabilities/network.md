# Network / Gateway Capability Reference

The cross-platform Capability is **Network / Gateway** and is currently `Defined` rather than fully normative.

## Identity rule

Network selection is a runtime/product choice, but chain-derived wallet state is always network-scoped.

The same Stellar address string may exist on more than one network, therefore an application must not treat the bare address as a globally unique durable account context.

Conceptually:

```text
Account context = Stellar network + account identity
```

Balances, liabilities, history, offers, transactions, anchor discovery/session state and caches must not leak across network boundaries.

## Configuration

A platform may expose network configuration in the form appropriate to its runtime, for example:

```toml
[network]
default = "testnet"
```

or an application preference/environment/profile.

Typical product networks include mainnet and testnet. Custom endpoints or future networks are implementation policy unless promoted into a stronger shared contract.

## Implementation boundary

The Capability requires access to Stellar network state and submission/protocol transport, but it does not mandate:

- Horizon vs RPC vs Portfolio API;
- one HTTP client;
- one retry strategy;
- one cache layout;
- one endpoint configuration format.

Those are platform mechanisms. Stable semantic results used by other Capabilities should be normalized before they reach Application Flows.

# Wallet Capability

Maturity: **Defined**

## Purpose

`Wallet` is the shared product concept used to group user-managed account/signer state and product metadata.

The capability is intentionally **Defined**, not Normative, because current products do not yet share one stable aggregate shape:

- the Rust engineering client still has a compact `WalletRecord` optimized for terminal use;
- the Mobile architecture uses distinct `AccountRecord`, `SignerRecord` and account/signer references;
- future products may group multiple accounts differently.

## Agreed boundary

Whatever local aggregate a product calls a wallet, it must not erase the shared identity rules:

```text
Account identity != Signer capability != Recovery source
```

Wallet/product storage may own:

- labels and local names;
- default/current selection;
- account/signer references;
- product metadata;
- network-scoped durable state;
- opaque protected signer envelopes where appropriate.

It must not redefine Core cryptography or make a software secret the identity of the wallet.

## Evolution rule

Do not standardize the Rust `WalletRecord` or the Mobile Realm schema as the cross-platform wallet contract.

Promote `Wallet` to Normative only after multiple product implementations reveal a stable aggregate model that is useful across runtimes.

# Wallet Capability

Maturity: **Defined**

## Purpose

`Wallet` is the shared product concept used to group user-managed account/signer state and product metadata.

The capability is intentionally **Defined**, not Normative, because current products do not yet share one stable aggregate shape:

- the Rust engineering client still has a compact `WalletRecord` optimized for terminal use;
- the Mobile architecture is moving toward distinct `AccountRecord`, `SignerRecord` and account/signer references;
- future products may group multiple accounts, signers or identities differently.

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

## Reference Semantics: Python and Rust terminal implementations

The current Python and Rust terminal implementations provide useful product evidence:

- [`reference/python/fresnica/wallet.py`](../../reference/python/fresnica/wallet.py)
- [`reference/python/fresnica/manager.py`](../../reference/python/fresnica/manager.py)
- [`reference/python/tests/test_wallet_state.py`](../../reference/python/tests/test_wallet_state.py)
- [`reference/python/tests/test_wallet_backup.py`](../../reference/python/tests/test_wallet_backup.py)
- [`reference/rust-client/src/wallet.rs`](../../reference/rust-client/src/wallet.rs)
- [`reference/rust-client/src/storage.rs`](../../reference/rust-client/src/storage.rs)

These implementations should be treated as evidence for candidate cross-platform semantics, not as a requirement to copy their record layout.

### 1. Public account identity survives signer lifecycle changes

Both implementations model watch-only as an account/product record without applicable local signing capability rather than as a different kind of Stellar address.

A Classic watch-only account may later attach matching local signing material. The signer identity is derived/verified before the durable record is upgraded. Detaching the local signer removes signing capability while preserving the account identity.

This lifecycle is already constrained by the Normative [Account](account.md) and [Signer](signer.md) contracts; the Wallet capability composes those semantics into a product-owned aggregate.

### 2. Lock state is capability/session state, not account existence

The Python reference distinguishes `WATCH_ONLY`, `LOCKED` and `UNLOCKED` product states. The account identity and readable public metadata continue to exist while signing capability is locked.

That distinction is useful across products:

```text
account exists / can be observed
        !=
local signer is currently authorized for use
```

The exact session model does not need to be shared, but a UI must not treat locking a signer as deleting or invalidating the account.

### 3. Default/current wallet selection is local product metadata

The terminal implementations support a default wallet and an explicit wallet selection fallback. This is useful product behavior, but it is not chain identity and must not affect the semantic meaning of the underlying account or signer.

Other platforms may use a current account, workspace, profile or navigation selection instead of a terminal-style default wallet.

### 4. Backup can preserve capability without revealing raw signing material

The Python and Rust references can back up a watch-only record without signing material and can persist protected software-signer state without first revealing the mnemonic or `S...` secret.

This behavior is now captured by the Defined [Backup / Restore Capability](backup-restore.md): watch-only backup must not invent signer material, protected signer backup must not declassify raw signing material merely for portability, and restore must validate the account/signer/recovery graph before activation.

The exact backup file format remains outside the Wallet Capability contract. The terminal v1 format is Reference Semantics for Backup / Restore rather than a required future Mobile/Desktop schema.

## Candidate semantics for promotion

The following behavior is worth validating in Mobile/Web/Desktop before promoting Wallet to Normative:

1. A wallet/product aggregate references account identity and optional signer capability rather than equating them.
2. Watch-only -> signer-attached -> watch-only transitions preserve account identity and network scope.
3. Lock/unlock changes local signer usability without changing public account truth.
4. Default/current selection remains product metadata rather than chain identity.
5. Wallet aggregation composes the separate Backup / Restore contract without redefining its security/activation semantics.

## Implementation-specific choices today

The following terminal choices are not cross-platform requirements:

- one account per `WalletRecord`;
- `wallet_type` string values;
- one active in-memory unlocked session;
- one terminal default-wallet file;
- the current JSON storage schema and hashed filenames;
- the field name `secret` for an opaque protected envelope;
- one global Fresnica passcode policy in the reference manager;
- the current backup JSON format/version;
- terminal commands and naming rules.

A Mobile Realm model, Web IndexedDB model or Desktop native store may look completely different while implementing the same candidate semantics.

## Promotion criteria

Promote Wallet to Normative only after materially different product implementations reveal a stable aggregate contract that is useful across runtimes. Do not standardize the Rust `WalletRecord`, Python `WalletRecord` or a Mobile Realm schema merely because one implementation is mature.

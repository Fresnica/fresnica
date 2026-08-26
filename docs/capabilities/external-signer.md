# Hardware Signer Integration

Status: **provider boundary defined; device transport implementation intentionally deferred until the selected provider can consume Fresnica's current transaction/XDR version without lossy conversion**.

## Core boundary stays unchanged

Hardware wallets are external signer providers. They do not receive a protected software-signer envelope or `WalletUnlockKey` and they do not require a new Core signing model.

The Fresnica flow remains:

```text
Fresnica SDK/Core prepare_ed25519_signing
  -> transaction hash + transaction XDR + network passphrase

hardware provider
  -> user reviews/approves on device
  -> 64-byte Ed25519 signature

Fresnica SDK/Core apply_ed25519_signature
  -> recompute hash
  -> verify expected signer public key/signature
  -> append decorated signature
```

Provider code owns device discovery, transport lifecycle, derivation-path selection and device UI/error translation. Core remains authoritative for Stellar transaction hashing and returned-signature verification.

## First provider candidate: Ledger

Ledger is the natural first hardware provider because the current Stellar Ledger application supports address display and Stellar transaction signing across current Ledger devices.

For Rust/native engineering clients, `stellar-ledger` is the preferred upstream starting point rather than implementing Stellar APDUs in Fresnica. Stellar CLI already uses this crate and opens the HID transport lazily for signing.

The default Stellar/SEP-5 account path is:

```text
m/44'/148'/account_index'
```

Fresnica should persist only provider identity/configuration such as provider kind, signer public key and account index. The private key never exists in Fresnica storage.

## Prefer full transaction signing

A Ledger provider should prefer the device's full Stellar transaction-signing operation when available. The device then receives the transaction plus network identity and can parse/display the transaction before approval.

`sign_transaction_hash` remains a fallback capability, not the default UX. Hash-only signing weakens on-device transaction review and can become blind signing. Regardless of which device operation is used, Fresnica still verifies the returned Ed25519 signature through `apply_ed25519_signature` before accepting it.

## Current dependency gate

As reviewed on 2026-08-25:

- Fresnica uses `stellar-xdr = 28.0.0` in the Rust CLI/current transaction layer.
- the current Stellar CLI workspace is `27.1.0`, including `stellar-ledger 27.1.0`, but it still consumes `stellar-xdr 27.0.0`;
- Ledger's current LedgerJS -> Device Management Kit migration documentation says Stellar does not yet have a chain-specific DMK signer kit; chains without a signer kit can use lower-level DMK commands;
- the existing JavaScript `@ledgerhq/hw-app-str` package remains an available Stellar application API, but it is part of the older LedgerJS family being migrated.

Do not solve this by downgrading Fresnica's XDR dependency or by introducing an implicit v28 -> v27 transaction conversion layer merely to claim hardware support. That creates a second transaction compatibility boundary and may fail as new XDR types appear.

The implementation gate is therefore one of:

1. a `stellar-ledger` release aligned with the XDR version used by Fresnica; or
2. a separately reviewed provider implementation that can feed the exact current transaction bytes to the Ledger Stellar app without reinterpreting/downgrading the transaction model.

## Platform rule

Hardware transport belongs above `fresnica-sdk`:

```text
application / framework
        |
hardware provider (Ledger, future providers)
        |
fresnica-sdk prepare/apply
        |
fresnica-core
```

Native, Web and React Native providers may use different transport libraries while preserving the same semantic signing request and verification boundary. Do not put HID/WebHID/BLE/USB dependencies in Core or the platform-neutral SDK.

## Validation target

When the dependency gate is resolved, the first hardware implementation should prove:

1. retrieve/display the Ledger-derived Stellar public key for a selected account index;
2. attach that public key as an external/hardware signer capability without a local envelope;
3. build a normal Fresnica transaction and call `prepare_ed25519_signing`;
4. approve the exact transaction on the device using full transaction signing;
5. pass the returned signature to `apply_ed25519_signature`;
6. reject a signature from a different Ledger account index/public key;
7. run the same flow against a Ledger emulator/Speculos before requiring a physical device in routine tests.

Until these checks pass, `Hardware transport adapters` remains incomplete even though the Core/provider boundary is already ready for them.

## Upstream references

- Ledger Stellar application: <https://github.com/LedgerHQ/app-stellar>
- Stellar `stellar-ledger` crate: <https://docs.rs/crate/stellar-ledger/latest>
- Stellar CLI Ledger integration: <https://github.com/stellar/stellar-cli/blob/main/cmd/soroban-cli/src/signer/ledger.rs>
- LedgerJS to Device Management Kit migration: <https://developers.ledger.com/docs/device-interaction/dmk-ts/integration/migrations/ledgerjs-to-dmk>

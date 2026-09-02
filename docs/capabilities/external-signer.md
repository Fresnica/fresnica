# Hardware / External Signer Interaction Capability

Maturity: **Defined**

Status: provider boundary defined; first RefPython Ledger physical Testnet compatibility gate verified.

## Purpose

The capability defines the common application boundary for invoking a signer whose private key is not a Fresnica protected software signer.

Examples include hardware wallets and other external Ed25519 providers.

## Core boundary

External signers do not receive a protected software-signer envelope or `WalletUnlockKey` and do not require a second Core signing model.

Preferred provider-neutral flow:

```text
Fresnica SDK/Core prepare_ed25519_signing
  -> transaction hash + exact transaction XDR + network context

external provider
  -> provider/user authorization
  -> 64-byte Ed25519 signature

Fresnica SDK/Core apply_ed25519_signature
  -> recompute exact hash/context
  -> verify expected signer identity/signature
  -> append verified decorated signature
```

Provider code owns discovery, transport lifecycle, account/path selection and provider-specific error/UI translation.

## Required boundary semantics

A conforming implementation must:

- persist only public/provider configuration needed to re-identify the signer;
- never require the external private key to enter Fresnica storage;
- bind provider output to the exact expected signer and transaction;
- verify the returned signature before accepting it as signed transaction state;
- prefer provider flows that allow meaningful transaction review over blind hash signing where available;
- keep USB/HID/BLE/WebHID/vendor dependencies above Fresnica Core/platform-neutral SDK.

## Reference design: Ledger

Ledger remains the first candidate hardware provider. The normal Stellar SEP-5 path is:

```text
m/44'/148'/account_index'
```

Fresnica should reuse maintained upstream Stellar/Ledger protocol libraries when version-compatible rather than implementing Stellar APDUs without need.

A provider becomes implementation-ready only when it can consume/sign the exact current Fresnica transaction representation through a deliberately reviewed boundary.

Do not solve a provider/XDR-version mismatch by downgrading Fresnica or introducing an implicit lossy transaction reinterpretation merely to claim hardware support. Exact current dependency/version status belongs in the project roadmap rather than this long-lived Capability contract.

This Ledger section is a **reference design and compatibility constraint**, not universal cross-device Reference Semantics.

## Implementation evidence status

RefPython carries an opt-in Ledger Stellar HID provider/probe over the existing Core prepare/apply boundary. Deterministic tests cover SEP-5 path packing, Ledger APDU chunking and fail-closed request binding.

The first physical-device compatibility gate succeeded on macOS on 2026-09-02 with the Ledger Stellar app **6.0.3**, default path `m/44'/148'/0'`, and Blind Signing disabled. The probe clear-signed and submitted a 1 XLM Testnet payment from `GB7FRMMSEU7NBDF64G5PVBIXPONG6ESDQTLGKZ6SVBA7XEJFFHQEVT3J` to `GB4QLYW37ZD2YSDJOENU5GZONPFNWGKO55E4XHDHJBHMKZ3IJ34Y7O6J`; the submitted transaction hash was `f91abc8bd8af37484bbb0c3c0e933e454df3131bb6b21598715e3af8f2beb4b0`, and the live pytest completed `1 passed`. Device model was not recorded, so this evidence proves the concrete Mac/HID/Stellar-app path rather than universal Ledger-model compatibility.

A future Ledger or other provider implementation may submit a documentation PR recording:

- signer/provider identity model and persisted public configuration;
- derivation/account selection semantics;
- exact prepare/provider/apply lifecycle used in practice;
- review capabilities and provider limitations;
- cancellation/disconnection/error behavior that proved product-significant;
- emulator/device regression evidence;
- proposed contract changes, if any.

The implementation may live in another Fresnica product repository. Source-code co-location is not required for its experience to mature this Capability.

## Validation target

A first hardware implementation should prove:

1. derive/display the selected provider public key;
2. attach it as an external signer without a local protected envelope;
3. prepare a normal Fresnica transaction;
4. obtain user-approved provider signature;
5. verify/apply the signature through SDK/Core;
6. reject a signature from another signer/account/path;
7. support repeatable emulator/provider tests before requiring physical devices for routine CI.

The RefPython Ledger proof now satisfies this first implementation target for the tested Mac/HID/Stellar-app path. Provider transport details remain platform-specific and can evolve without changing this Defined capability boundary.

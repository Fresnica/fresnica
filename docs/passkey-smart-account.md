# Fresnica Passkey / Smart Account Contract

Updated: 2026-08-25

## Decision

Fresnica should support passkeys as a **contract-account authorization model**, not as a convenience replacement for the `WalletUnlockKey` of an existing protected Ed25519 software signer.

The first interoperability target is Stellar's `smart-account-kit` / OpenZeppelin smart-account model rather than the older single-purpose `passkey-kit` model.

This is an integration target, not a Rust Core dependency. The TypeScript kit may change independently; Fresnica must pin and adapt it at the product/provider boundary rather than importing its API semantics into the universal Core contract.

## Why this is a different signer model

A passkey smart wallet is fundamentally different from the current classic-account software signer:

```text
Classic software wallet
  Account G...
  Signer  Ed25519 G...
  private S... / mnemonic protected by Fresnica envelope
  transaction-envelope Ed25519 signature

Passkey smart wallet
  Account C...
  Signer  contract-defined external signer
  private secp256r1 credential remains inside WebAuthn authenticator
  Soroban contract authorization / verifier contract
```

Therefore:

- do not derive a Fresnica `WalletUnlockKey` from a WebAuthn assertion;
- do not wrap a `WalletUnlockKey` with a passkey and call that a passkey wallet;
- do not force passkey authorization through `prepare_ed25519_signing` / `apply_ed25519_signature`;
- do not treat the `C...` account address as a signer public key;
- do not silently convert a watch-only `G...` account into a passkey account.

Moving from a classic `G...` wallet to a smart `C...` wallet is an account/product migration, not signer attachment to the same account identity.

## Upstream model to reuse

The current Stellar `smart-account-kit` model is a stronger fit for Fresnica than building a new smart-account contract because it already provides:

- WebAuthn passkey external signers using secp256r1;
- Ed25519 external signers;
- delegated classic Stellar accounts;
- multiple signers;
- context rules bound into the authorization digest;
- threshold / weighted-threshold / spending-limit policy examples;
- fee-sponsored submission support;
- an OpenZeppelin smart-account contract base.

This maps well to Fresnica's existing rule that Account identity and Signer capability are separate.

Fresnica should reuse the deployed contract/verifier semantics and shared Stellar primitives rather than fork the smart-account contract merely to make it "ours".

## Authority boundary

For protected Ed25519 software signers, Fresnica Core is the cryptographic authority.

For a passkey smart account, the authority is split differently:

```text
Fresnica application/service layer
  - transaction intent and review
  - RPC/ledger state
  - account/provider selection
  - relayer/fee-payer policy

smart-account provider adapter
  - smart-account contract interaction
  - context-rule resolution
  - WebAuthn ceremony orchestration
  - contract-specific authorization encoding

platform/browser authenticator
  - owns secp256r1 private credential
  - produces WebAuthn assertion

on-chain smart account + verifier contract
  - authoritative authorization verification

Fresnica Core / universal SDK
  - remains authoritative for Fresnica-owned Ed25519 signer/envelope semantics
  - may provide generic XDR/review primitives where they are truly protocol-generic
  - must not pretend to own inaccessible passkey private material
```

Core must not call browser WebAuthn, Android Credential Manager, Apple AuthenticationServices, or equivalent platform APIs.

## Account and signer persistence

Do not overload `ProtectedSoftwareSignerRecord` or its `signerPublicKey: G...` field for passkeys.

A future passkey signer record needs a separate provider-specific shape. At minimum it must distinguish:

```text
PasskeySignerRecord
  id                    application-local stable id
  credentialId          WebAuthn credential identifier / discovery metadata
  verifierAddress       C... verifier contract
  keyData               public contract signer data required by the verifier
  provider              smart-account-kit / compatible provider id
```

The exact serialized `keyData` format should remain provider-owned/opaque unless Fresnica needs to interpret it for a protocol-level reason.

The smart account remains an ordinary `AccountRecord` with `kind = contract` and `address = C...`.

On-chain context rules, signer sets and policy state are ledger authority. Local persistence may cache them for UX, but must not become a second authoritative policy database.

## Transaction authorization flow

The passkey prompt must authorize the transaction the user actually reviewed.

Conceptually:

```text
1. build/simulate smart-account transaction
2. resolve exact Soroban authorization entries + context rules
3. present human review
4. freeze the reviewed authorization payload
5. invoke passkey/WebAuthn over the provider's exact auth digest
6. attach the assertion to the authorization entry
7. submit using selected fee payer / relayer
8. verify resulting ledger outcome
```

Do not authenticate first and then allow application code to replace the reviewed transaction or authorization context.

`smart-account-kit` Protocol-27 authorization binds context-rule IDs into the digest. Fresnica should preserve that property rather than reducing the flow to a generic "passkey succeeded" boolean.

## Relayer is not signer custody

A relayer/fee payer may pay transaction fees, but it is not the user's passkey signer and must not be modeled as account custody.

Fresnica should keep these roles separate:

```text
account owner / signer  -> user passkey or other configured smart-account signer
fee payer / relayer     -> service or user-selected fee source
```

Loss of the Fresnica relayer service must not imply loss of the user's smart account. Where the selected smart-account/provider model permits it, direct RPC submission with another fee payer should remain possible.

## Recovery

A passkey smart wallet should use smart-account recovery primitives rather than secretly generating a mnemonic behind the passkey.

Potential recovery options include:

- second passkey / second device;
- delegated `G...` account controlled by the user;
- Ed25519 external recovery signer;
- threshold/weighted policy involving multiple user-controlled signers.

The product must make recovery policy explicit during onboarding. A single-device passkey with no recovery signer is a valid but materially different risk profile from a mnemonic-backed classic account.

## Web

For Web, the current `fresnica-wasm-sdk` continues to serve classic protected Ed25519 accounts through fresh-passcode signing.

Passkey smart accounts should be added as a separate smart-account provider path. Do not expand the WASM SDK with fake `WalletUnlockKey` passkey APIs.

Initial Web layering:

```text
Web wallet
  +-- classic G... software signer -> fresnica-wasm-sdk
  +-- smart C... passkey account   -> smart-account provider adapter
```

This allows both account models to coexist without weakening either security boundary.

## Mobile

Mobile should target the same on-chain smart-account/verifier semantics, but the passkey ceremony may be platform-native rather than browser JavaScript.

The React Native adapter must not own smart-account cryptography. A future Mobile provider may bridge Android/Apple passkey APIs and return only the assertion/public authorization material required by the smart-account provider.

Do not assume a browser-only WebAuthn implementation is sufficient for React Native.

## Compatibility and release rules

`smart-account-kit` is currently pre-1.0 and its package API may change. The first Fresnica provider prototype pins upstream package **0.6.2** and the upstream Protocol 27 Testnet deployment published on **2026-07-09**. The upstream project currently states that its SDK/demo/relayer integration has not undergone an independent security audit, so this checkpoint is Testnet-only and is not evidence for production/mainnet enablement. Fresnica must separately pin/verify package versions and on-chain contract/verifier deployments for every release.

Before production enablement, record at minimum:

- smart-account-kit version;
- smart-account contract WASM hash/version;
- WebAuthn verifier contract address/hash;
- deterministic deployer identity/derivation if used;
- supported Stellar network/protocol version;
- context-rule/policy compatibility;
- known upstream security caveats and migration path.

The upstream deterministic-deployer design documents an address-squatting residual if a credential ID is learned before intended deployment. Fresnica must explicitly review and accept/mitigate that risk before production rather than hiding it behind SDK abstraction.

## Implementation sequence

1. Keep the current classic `G...` software-signer path unchanged.
2. Add a provider-neutral **contract smart-account capability** at the application/service boundary, not in `ProtectedSoftwareSigner`. **Implemented:** `providers/smart-account-kit` provides the first pinned provider boundary with create/connect/discover and safe sign-and-submit orchestration.
3. Run the provider against real Testnet WebAuthn in a browser and capture create/connect/sign-and-submit results. **Harness ready:** the smoke page records only the confirmed relayer `func/auth` XDR and validates Protocol-27 digest/context binding plus the WebAuthn P-256 signature before fixture export.
4. Persist passkey credential/provider metadata separately from protected Ed25519 signer records.
5. Add conformance fixtures from real smart-account transaction/auth XDR, including context-rule identity. **Verifier ready; real fixture still pending:** `npm run fixture:verify -- <fixture.json>` performs the same independent digest/context/WebAuthn verification offline.
6. Only after the provider boundary is proven, decide which protocol-generic Soroban authorization helpers belong in Rust Core/SDK.
7. Add platform-native Mobile passkey adapters against the same smart-account contract semantics.

## Non-goals

This design does not:

- replace existing mnemonic/secret wallets;
- make every `C...` address a passkey wallet;
- make WebAuthn a universal Fresnica app-unlock mechanism;
- introduce a new Fresnica smart-account contract;
- move network/RPC state into Rust Core;
- expose authenticator private keys or `WalletUnlockKey` material to JavaScript.

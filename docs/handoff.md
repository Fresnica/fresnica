# Fresnica Project Handoff

Updated: 2026-08-26

This is the compact continuation document for Fresnica. It records the current architecture, product invariants, SDK direction, Mobile handoff boundary and the next major work. Read `roadmap.md` together with this file before starting a new phase.

## Repository State

- Repository: `manran/fresnica`
- Default branch: `main`
- Verified Mobile/SDK architecture baseline: PR #109 / `0de8be490ba2e136324ddfbc737d2eadace5e12a`; final consumable baseline is `native-sdk-v0.2.1`, whose exact target commit is recorded in its release manifest.
- PR #90 introduced the platform-neutral `fresnica-sdk` semantic contract.
- PR #91 converted the Mobile v0.1.0 UniFFI facade into a compatibility wrapper over `fresnica-sdk`.
- PR #92 introduced the generalized `fresnica-native-sdk` UniFFI layer plus framework-neutral Android AAR and Apple package/XCFramework generation.
- Subsequent SDK/platform validation completed the WASM package, smart-account Testnet conformance vector, Apple/macOS Native SDK validation and the React Native 0.87 Apple consumer gate.
- Rust CLI Phase-5 work subsequently added verified SEP-10 sessions, SEP-24/SEP-6 transfer initiation, transaction status and reviewed withdrawal-payment handoff while keeping signing on the SDK/Core path.
- PR #107 published the first generalized `native-sdk-v0.1.0` prerelease. PR #109 finalized the Mobile security/HD contract and published `native-sdk-v0.2.0`; v0.2.1 is the corrective handoff release with transactional domain cleanup fixes and synchronized Mobile documentation after the full 13-workflow PR matrix, Android raw-AAR standalone consumer, Apple iOS/macOS Native SDK, and Android/Apple native-signing gates all passed.
- The published v0.2 release target is exactly `0de8be490ba2e136324ddfbc737d2eadace5e12a`; independently downloaded Android/Apple workflow artifacts matched the GitHub Release SHA-256 metadata (`7fbb0b9d...e5ee78d` AAR, `672255f5...8c3824` Apple zip).
- `native-sdk-v0.2.1` is the Mobile baseline: Native Binding API 2 / Universal SDK API 3 / Core Client API 3 / RN adapter source 0.2.0. `mobile-sdk-v0.1.0` and `native-sdk-v0.1.0` remain historical compatibility material.

Do not treat a SHA in this handoff as the permanent head. Verify `main` and current CI before writing.

## Current Strategic Direction

The project is now **SDK-first and multi-platform**.

The near-term sequence is:

```text
Rust Core
  -> stable universal SDK contract
  -> compiled Native SDKs / WASM package
  -> canonical framework adapter source
  -> one-time consumer-side adapter compilation
  -> reusable wallet functional coverage / SEP alignment
  -> Mobile / Desktop / Web wallet experience
```

The universal SDK, generalized Native SDK release and canonical one-time React Native adapter path are now established. The independent React Native product can start from `mobile-sdk-usage.md` without waiting for more Core infrastructure. This repository continues reusable SDK/Core/SEP/provider work; `fresnica-mobile` owns product/application development.

## Target Layering

```text
                         Rust Core
                            |
                            v
               Stable SDK API / DTO / errors
                    /                 \
                   /                   \
          Native SDK binaries          WASM SDK
      Android / Apple / Win /          Web/browser
          Linux / macOS                   |
                   |                      |
             native application       web application
                   |
          framework adapter source
       RN / Flutter / Electron / ...
                   |
        compile once in consumer env
                   |
          generated adapter binary
                   |
             product application
```

The shared contract is semantic, not a requirement that every target use the same binary format.

## Universal SDK Rules

The platform-neutral `sdk/rust` contract is now the semantic owner above Rust Core. `bindings/mobile` is a compatibility facade for the v0.1.0 Mobile surface; `bindings/native` is the generalized UniFFI/native binding layer. Do not create separate Mobile and Desktop crypto semantics.

The SDK contract should own:

- stable wallet/signer operations backed by `CoreClientApi`;
- DTOs and stable error categories;
- Account / Signer identity semantics;
- protected software signer envelope semantics;
- transaction hash/signature semantics;
- API version reporting;
- shared conformance vectors/tests.

The SDK contract should **not** absorb application persistence, framework lifecycle or arbitrary platform UI behavior.

### Native SDK release rule

Application projects should consume **compiled platform SDK content**. Ordinary application builds must not compile Rust Core or run UniFFI generation.

Current platform outputs:

- Android: complete direct-consumer AAR
- Apple: Rust FFI XCFramework plus importable `FresnicaSDK.xcframework`; the complete `validate-apple-local.sh` flow passed on real macOS/Xcode on 2026-08-25, including independent consumer import/typecheck

Desktop status:

- macOS: `FresnicaSDK.xcframework` / `FresnicaSDKFFI.xcframework` include universal macOS slices and the macOS Data Protection Keychain path; the expanded Apple validator passed on real macOS/Xcode on 2026-08-25
- Windows: compiled native library/package waits for a defined direct-consumer language/API surface
- Linux: compiled native library/package waits for a defined direct-consumer language/API surface

Android native applications can use the AAR directly without React Native/Flutter/etc. The Apple iOS and macOS direct-consumer package paths are validated on real macOS/Xcode. Do not describe a bare desktop UniFFI `.dll`/`.so` as a complete public SDK until its supported direct-consumer language/API surface is defined.

### WASM / Web rule

Web should receive a WASM-facing SDK preserving the same Core semantics where appropriate.

Do not copy Mobile security assumptions blindly into browsers. Browser key protection/storage/authorization needs an explicit security design; `WalletUnlockKey` + Keychain/Keystore behavior is not automatically the Web model.

Web may consume WASM directly. Add framework glue only where a real framework integration requires it.

Passkey smart accounts are explicitly **not** a WebAuthn wrapper around `WalletUnlockKey`. `passkey-smart-account.md` defines them as `C...` contract accounts with provider/on-chain authorization. The first interoperability target is Stellar `smart-account-kit`; keep that integration outside protected Ed25519 signer records and outside Core platform-auth APIs. The provider boundary under `providers/smart-account-kit` is pinned to upstream 0.6.2 plus the published 2026-07-09 Protocol 27 Testnet deployment. Local mock conformance is green, and on 2026-08-25 the localhost browser harness completed a real WebAuthn/Testnet create/connect/sign-and-submit flow. Its verified auth-XDR/context-rule fixture is checked in as `spec/test-vectors/smart-account-auth-v1.json`.

## Framework Adapter Contract

Authoritative document: `mobile-framework-adapter-contract.md`.

Fresnica owns and publishes **canonical adapter source/reference implementation**. The framework adapter is not part of the Native SDK binary.

Consumer behavior is fixed as:

```text
first integration
or framework/binding incompatibility
    -> compile canonical adapter once against consumer framework/toolchain
    -> store/version generated adapter binary + compatibility manifest

normal app build
    -> link Native SDK binary
    -> link generated adapter binary
    -> no Rust/Core rebuild
    -> no adapter-source rebuild
```

For Mobile, React Native is the first adapter target. Canonical Android/Apple adapter source lives under `adapters/react-native` and targets the generalized Native SDK. Both platforms have one-time consumer build commands plus compatibility manifest/rebuild checks. The underlying Apple `FresnicaSDK` module is validated on real macOS/Xcode, and the complete static Apple adapter path has passed `validate-consumer.sh` against a freshly generated React Native 0.87 CocoaPods consumer on macOS. Modern prebuilt React Native is resolved from CocoaPods' installed `React.xcframework` plus public/private header roots rather than by reconstructing `node_modules` header layouts. Flutter/Desktop adapters should follow the same architecture when implemented.

An adapter may perform argument/result conversion, Promise/Future/callback mapping, thread dispatch and framework registration. It must not own cryptography, envelope mutation, signer identity verification, transaction signing logic or native key-protection policy.

Rule:

> **Adapter = mechanical framework glue. SDK/Core = security authority.**

## Core and Signer Model - Do Not Regress

The established model is:

> Wallet/Account is the on-chain identity the user observes/operates. Signer is a currently available signing capability. They are not the same object and the relationship is not necessarily one-to-one.

Consequences:

- a classic account `GABC...` may use signer `GDEF...` under Stellar signer/threshold rules;
- the master-key case commonly has account and signer identities equal, but that is not a universal invariant;
- a watch-only account has no local signer capability and requires no passcode, mnemonic, secret or protected envelope;
- watch-only identity parsing still goes through Core/SDK;
- direct master-key watch-only upgrade must identity-check the supplied secret/mnemonic against the expected account `G...`;
- `C...` is an account/contract identity, not an Ed25519 public key;
- do not pretend an arbitrary `S...` directly owns a `C...` account; contract/passkey authorization is a separate future auth model.

For application persistence, the durable conceptual graph remains:

```text
AccountRecord
  identity/address/network/product metadata

SignerRecord
  signer public identity
  signer kind/provider metadata
  protected envelope when applicable

AccountSignerReference
  account <-> signer relationship
```

Watch-only is derived from absence of an applicable local signer reference, not a second drifting wallet-type truth.

## Secret and Native Authorization Boundaries

Rust Core remains authoritative for:

- secret/mnemonic validation and derivation;
- signer identity;
- protection/envelope cryptography;
- passcode re-protection;
- transaction hashing/signing;
- external Ed25519 signature verification;
- stable crypto/error semantics.

Client/platform code owns:

- persistence;
- application lock/session state;
- Keychain/Keystore/platform credential lifecycle;
- network/Horizon state;
- ledger signer weights/threshold authorization;
- product UX.

Security invariants:

- **Passcode > System Auth**: system authentication may authorize routine signing but cannot substitute for the Fresnica passcode, Reveal/Export, passcode rotation, or recovery authority;
- **Account != Signer != Recovery Source**; mnemonic recovery-source grouping is Mobile-owned UX metadata while each signer retains an independent Core envelope;
- Mobile initializes at most one device System Auth Protection Domain; later signers are passcode-verified and wrapped with the domain public key without another biometric prompt;
- a mnemonic-backed protected source may derive another explicit index through Core/SDK `deriveMnemonicSigner` without returning the mnemonic to JavaScript;
- routine protected-software signing remains native-only;
- `WalletUnlockKey` does not enter JavaScript/Dart;
- raw low-level `signTransactionXdr` is not exposed to normal framework code;
- Reveal/Export requires a fresh Fresnica app passcode, not stored system-auth material;
- protected signer envelopes are opaque to application code;
- plaintext secret/mnemonic is exceptional and ephemeral: import, one-time generation/backup, explicit Reveal/Export only;
- plaintext recovery material must not be persisted in Realm/application state/logs/analytics.

## Mobile Status and Repository Boundary

The independent Mobile product should not be implemented further inside `fresnica`.

`fresnica` owns:

- Rust Core;
- universal SDK contract/facade;
- compiled native platform SDK release machinery;
- platform-native signer authorization/security adapters where part of the SDK layer;
- canonical framework adapter source/build tooling;
- conformance tests/vectors;
- SDK documentation.

Future `fresnica-mobile` owns:

- React Native application;
- navigation/screens/product state;
- actual Realm configuration/migrations;
- Account/Signer persistence;
- network/Horizon integration;
- create/import/watch-only UX;
- passcode/settings UX;
- Reveal/Export UX;
- hardware signer UX.

PR #81-#84 application-side TypeScript remains migration/reference material until the independent Mobile project absorbs the required behavior. See `mobile-app-migration-pr81-pr84.md`.

The Mobile onboarding order is now executable and documented in `mobile-sdk-usage.md`:

1. pin an exact React Native version;
2. pin `native-sdk-v0.2.1` / Native Binding API 2 / matching RN adapter source 0.2.0;
3. checksum-verify the Android/Apple release artifacts;
4. compile the canonical RN adapter once in the Mobile environment and store adapter binaries + compatibility manifest;
5. prove `FresnicaCore.parseAccount` from React Native on Android and iOS;
6. establish one Fresnica app passcode and optionally call `initializeSystemAuth` once for the device;
7. create/import/derive signers with passcode verification and, when the domain exists, call `registerSignerSystemAuth` without another biometric prompt;
8. absorb Account/Signer/Recovery-Source/Realm/passcode/export application flows.

## Desktop Direction

Desktop is no longer a separate architecture problem. It should use the same universal SDK contract.

The direct-consumer contract is now explicit in `desktop-sdk-contract.md`:

- Rust desktop clients consume `fresnica-sdk` directly;
- macOS Swift reuses the compiled `FresnicaSDK` packaging; the universal macOS slices passed real-Xcode validation on 2026-08-25;
- Windows/Linux non-Rust packages are product-language driven rather than declared as generic `.dll`/`.so` SDKs;
- UniFFI's internal C-compatible layer is not a stable Fresnica public C ABI.

Framework-based apps use thin adapters when needed, for example Electron/Node, Flutter Desktop, Qt or .NET, after the direct-consumer language/framework is selected.

Desktop platform key protection remains platform-specific, e.g. Windows DPAPI/Windows Hello and Linux Secret Service/libsecret where appropriate. These implementations must preserve the same signer/security contract instead of reimplementing Core crypto.

## Rust Engineering Clients

The Rust CLI is substantially implemented and remains the reference native command client for Core/SDK behavior. Its account identity, wallet protection, Reveal/Export and routine passcode-signing paths consume `fresnica-sdk`; CLI/TUI presentation code no longer imports Core directly. The remaining direct Core use is contained inside the shared Rust client layer for low-level transaction/XDR helpers and mnemonic-language detection where no SDK operation is currently warranted.
It also covers Classic watch-only upgrade/downgrade: attaching S/mnemonic material is identity-bound through the SDK expected-signer check, while detaching removes local signer material and preserves the G account record.
The Phase 5 reference-client path includes SEP-1 discovery, SEP-10 Classic-account sessions, SEP-24-preferred / SEP-6-fallback deposit/withdraw initiation, transaction-status tracking and an explicit reviewed withdrawal-payment handoff. SEP-45 contract-account execution and SEP-12 customer-information handoff remain separate follow-up work.

A reusable Rust application layer now lives at `clients/rust-client` (`fresnica-client`). It is deliberately above the universal SDK and below terminal presentation. The extracted surface owns the existing Rust engineering-client wallet storage/lifecycle, contacts, Horizon transport, account/balance/history services, UI-free transaction orchestration, pending-transaction protection, payment/trustline prepare-review-submit semantics, SDEX offer create/update/cancel preparation/submission, and typed open-offer reads. Shared service DTOs should remain transport-neutral; the SDEX `OpenOffer` surface intentionally exposes normalized wallet semantics rather than Horizon raw JSON. It does **not** become a new crypto authority: protected signer semantics and routine signing still route through `fresnica-sdk`, while application persistence/network orchestration remain client-layer concerns.

Mobile should mirror this same application-layer shape rather than inventing a separate product architecture: React Native screens/state -> Mobile application services -> Realm/network/platform adapters + Fresnica Native SDK. Reuse the service responsibilities and wallet semantics where they are common, but do not require Mobile to link the concrete `fresnica-client` Rust implementation; persistence technology, platform lifecycle and product orchestration remain application-owned.

The native Rust TUI lives at `clients/rust-tui`. It consumes `fresnica-client` rather than importing CLI command handlers. Its current reference slice includes:

- selected wallet identity and watch-only/local-signer capability;
- network-scoped wallet switching for the current TUI session;
- balances/liabilities;
- recent account activity;
- manual refresh;
- reviewed XLM/issued-asset payment preparation through the shared payment service;
- masked passcode entry and SDK-backed payment submission, including shared pending-transaction protection;
- reviewed trustline add/limit/remove preparation and submission through the same shared client layer;
- reviewed SDEX BUY/SELL offer creation, offer update and offer cancellation through shared offer services;
- typed current open-offer display from the shared SDEX read service.

Richer pair/orderbook screens remain follow-up TUI work. Terminal interaction, confirmation state and passcode entry remain presentation-owned and must not be copied from CLI command handlers.

## Python Wallet Reference

The Python implementation remains valuable for stable wallet behavior and UX/reference semantics, including:

- mnemonic/secret/watch-only wallets;
- encrypted signing material;
- wallet lifecycle and backup/export;
- balance/portfolio;
- transaction send/review/submit;
- history/cache;
- contacts;
- assets/trustlines;
- SDEX;
- anchor transfer workflows;
- Textual TUI behavior.

It should continue to provide behavior examples/test vectors where Rust functionality has not yet fully replaced it. It is no longer a reason to keep new architecture Python-specific.

## Stellar / Wallet Functional Direction

Continue filling wallet fundamentals while SDK work proceeds. Reusable wallet behavior should live below final Mobile/Desktop/Web UI.

Priority domains:

- account/signer/watch-only lifecycle;
- balances and reserve/liability-aware availability;
- payments/transaction review;
- assets/trustlines;
- history/activity;
- contacts;
- SDEX offers/markets;
- network configuration;
- hardware/external signers;
- anchor/SEP workflows.

Use official Stellar SDK/protocol primitives rather than duplicating encoding/signing/transaction behavior.

Full asset identity remains authoritative: classic issued assets are `CODE:GISSUER`, never code-only, and chain-derived state remains scoped by Stellar network.

## SEP / Anchor Policy

The anchor separation remains:

- `AnchorService`: protocol/transport operations such as SEP-1, SEP-10, SEP-6, SEP-24;
- `AnchorTransferService`: wallet-facing protocol selection, field planning and response interpretation.

Current reference behavior:

- usable SEP-24 deposit/withdraw is supported in the Python reference;
- SEP-6 deposit/withdraw is supported in the Python reference;
- Rust CLI now covers SEP-1 + SEP-6/SEP-24 capability discovery while preserving full `CODE:GISSUER` identity;
- usable SEP-24 is preferred, SEP-6 is fallback;
- SEP-10 uses official Stellar signing/verification semantics;
- memo text/id/hash/return-hash is handled through the reviewed transfer path;
- KYC-required responses are surfaced;
- full generic SEP-12 collection is not yet implemented.

Policy going forward:

> Follow Stellar SEP standards and official ecosystem behavior where applicable; do not invent product-specific protocol substitutes merely to make a UI flow appear complete.

SEP-12 or other SEPs should be added when product requirements justify them and should remain protocol/service behavior below the UI.

## Existing Wallet Product Invariants

These Python/reference behaviors remain valid unless explicitly redesigned.

### History

- normal local cache retains the newest 2,000 Horizon operations per account/network;
- empty-cache initialization pages backward from current head;
- existing cache synchronizes forward from newest local paging token;
- full-history opt-in disables trimming and may backfill older records still retained upstream;
- History presentation is derived from cached raw operations so metadata/contact changes can redraw without rebuilding cache.

Do not reintroduce the old fixed `5 x 200` catch-up model.

### SDEX / DEX

- DEX is a wallet-oriented SDEX terminal, not a separate exchange engine;
- market identity uses full assets and network/wallet scope;
- current pair data has REST fallback and SSE augmentation;
- BID/BUY is left, ASK/SELL is right;
- order book presentation is `Amount | BID Price || ASK Price | Amount`;
- BID amount normalization uses exact Horizon `price_r`;
- Stellar price display keeps fixed 7-decimal semantics without rendering nonzero values as false zero.

### Trustlines

Normal user terminology is **Manage Assets**. New Fresnica-created trustlines currently use the visible marker limit:

`708269837873.6765`

Do not rewrite existing user trustline limits automatically and do not change this marker without an explicit product decision.

## CI policy

Validation workflows are PR/manual only so branch pushes and merges to `main` do not duplicate expensive builds. The lightweight `Main bundle` workflow is the deliberate normal `main`-push exception: it creates/verifies `fresnica-main.bundle`, uploads it as an Actions artifact, and publishes a `main-bundle` commit status for automated discovery. The marker-gated **Native SDK** release workflow is the other intentional heavy exception when a `releases/native-sdk-v*.json` release intent changes. The legacy Mobile SDK publisher is retired from active `main` workflows.

`sdk/compatibility/manifest.json` now records the compatible Core/SDK/Native/Mobile/WASM/React-Native version set. Run `node sdk/compatibility/validate.mjs` after changing any API/version constant or adapter contract; the matching GitHub check is lightweight and PR/manual-only.

## Smart-account conformance capture checkpoint

`providers/smart-account-kit` now has a real-Testnet auth-XDR recorder/verifier path. The browser smoke harness captures only the public relayer `func/auth` payload for a confirmed transfer, verifies the Protocol-27 signature payload/auth digest, extracts `context_rule_ids`, checks the WebAuthn challenge, and verifies the compact P-256 signature from the on-chain External signer key data. It then enables fixture download. The CLI verifier is `npm run fixture:verify -- <fixture.json>`.

Real-browser validation exposed upstream `smart-account-kit` 0.6.2 requesting WebAuthn `userVerification: "preferred"` while the deployed verifier rejects UV=0 (3117). Fresnica injects a required-UV WebAuthn adapter and the fixture verifier checks both UP and UV flags. The rerun succeeded on Testnet; its confirmed auth data is now the canonical `spec/test-vectors/smart-account-auth-v1.json` vector.

## Immediate Next Work

The infrastructure handoff is now split cleanly between SDK maintenance and product work:

1. **Mobile can start independently**: follow `mobile-sdk-usage.md`, pin `native-sdk-v0.2.1`, compile/store the RN adapter binaries once, pass the compatibility check and prove `FresnicaCore.parseAccount` on Android/iOS.
2. **Mobile owns product/application migration**: absorb #81-#84 Account/Signer/Recovery-Source/Realm/provisioning/passcode/export behavior in `fresnica-mobile`; use one device System Auth Domain and the v0.2 `deriveMnemonicSigner` contract without moving crypto or WalletUnlockKey policy into JavaScript.
3. **Fresnica keeps SDK evolution additive/released**: Core/SDK/native changes continue here and reach Mobile only through reviewed release/version contracts; normal Mobile builds never compile Rust/UniFFI.
4. **Wallet standards continue below product UI**: SEP-12 customer-information handoff is the next unblocked Anchor gap; SEP-45 remains the contract-account auth path.
5. **Smart account / passkey**: keep the real Testnet conformance vector; implement platform-native Mobile provider integration when the Mobile product reaches that feature.
6. **Hardware signer**: retain the external-signer prepare/apply boundary and wait for a compatible Ledger transport/XDR path instead of forcing conversion.
7. **Reference/other clients**: keep Rust CLI current; Desktop/Web reuse the same stable SDK architecture.

## Start Here Next Session

1. Verify `main` HEAD and relevant CI/release status.
2. Read `roadmap.md` for phase order.
3. Read `mobile-sdk-usage.md` to start the independent Mobile project; read `mobile-framework-adapter-contract.md` before changing Native SDK or RN packaging.
4. Read `mobile-app-migration-pr81-pr84.md` before starting the independent Mobile application.
5. Read `client-core-security.md` and `mobile-core-contract.md` before changing signer/passcode/system-auth boundaries.
6. Preserve Account != Signer != Recovery Source, `Passcode > System Auth`, `C...` identity semantics, native-only routine signing and fresh-passcode-only Reveal/Export.
7. Treat `mobile-sdk-v0.1.0` and `native-sdk-v0.1.0` as frozen history; new Mobile work pins `native-sdk-v0.2.1` and its generated RN adapter binaries.
8. Do not start product-specific Desktop/Web framework code until the universal SDK contract has a stable shape.

# Fresnica Project Handoff

Updated: 2026-08-25

This is the compact continuation document for Fresnica. It records the current architecture, product invariants, SDK direction, Mobile handoff boundary and the next major work. Read `roadmap.md` together with this file before starting a new phase.

## Repository State

- Repository: `manran/fresnica`
- Default branch: `main`
- Verified `main` before this handoff update: `89b20e79ed045fecef94051ec99a5ecb6eab692f` (PR #95).
- PR #90 introduced the platform-neutral `fresnica-sdk` semantic contract.
- PR #91 converted the Mobile v0.1.0 UniFFI facade into a compatibility wrapper over `fresnica-sdk`.
- PR #92 introduced the generalized `fresnica-native-sdk` UniFFI layer plus framework-neutral Android AAR and Apple package/XCFramework generation.
- Subsequent SDK/platform validation completed the WASM package, smart-account Testnet conformance vector, Apple/macOS Native SDK validation and the React Native 0.87 Apple consumer gate.
- PR #95 hardened the Rust CLI validation path: the SDK-boundary guard is portable on the GitHub Ubuntu runner, the Horizon submission mock consumes complete request bodies, Rust unit tests pass 52/52, the release binary builds, and Python/Rust CLI compatibility passes 5/5.
- `mobile-sdk-v0.1.0` remains a transitional compatibility baseline; new native consumers should target `fresnica-native-sdk`.

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

The immediate priority is **not** to continue building the actual React Native product inside this repository. The universal semantic contract now exists; current work is to finish native platform packaging/security support, then build the canonical one-time framework-adapter path and WASM boundary.

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

The future Mobile onboarding order is:

1. pin React Native version;
2. pin Fresnica Native SDK / Binding API;
3. compile the canonical RN adapter once in the Mobile environment;
4. store adapter binaries + compatibility manifest;
5. prove a smoke operation such as `parseAccount`;
6. then absorb Account/Signer/Realm/provisioning/passcode/export application flows.

## Desktop Direction

Desktop is no longer a separate architecture problem. It should use the same universal SDK contract.

The direct-consumer contract is now explicit in `desktop-sdk-contract.md`:

- Rust desktop clients consume `fresnica-sdk` directly;
- macOS Swift reuses the compiled `FresnicaSDK` packaging; the universal macOS slices passed real-Xcode validation on 2026-08-25;
- Windows/Linux non-Rust packages are product-language driven rather than declared as generic `.dll`/`.so` SDKs;
- UniFFI's internal C-compatible layer is not a stable Fresnica public C ABI.

Framework-based apps use thin adapters when needed, for example Electron/Node, Flutter Desktop, Qt or .NET, after the direct-consumer language/framework is selected.

Desktop platform key protection remains platform-specific, e.g. Windows DPAPI/Windows Hello and Linux Secret Service/libsecret where appropriate. These implementations must preserve the same signer/security contract instead of reimplementing Core crypto.

## Rust CLI and Possible Rust TUI

The Rust CLI is substantially implemented and should remain a reference native client for Core/SDK behavior. Its account identity, wallet protection, Reveal/Export and routine passcode-signing paths now consume `fresnica-sdk`; direct Core use is limited to low-level Rust transaction/XDR helpers and mnemonic-language detection where no SDK operation is currently warranted.
It also covers Classic watch-only upgrade/downgrade: attaching S/mnemonic material is identity-bound through the SDK expected-signer check, while detaching removes local signer material and preserves the G account record.
The Phase 5 reference-client path now starts absorbing anchor behavior as well: `anchor discover CODE:GISSUER` resolves the issuer `home_domain`, loads SEP-1 `stellar.toml`, verifies exact asset identity and probes SEP-6 / SEP-24 `/info`. This is capability discovery only; authenticated SEP-10/SEP-45 transfer execution has not been moved into the Rust client yet.

A Rust TUI is worth considering after the universal SDK boundary is clear. Its role should be engineering-focused:

- SDK integration proving ground;
- wallet-flow playground;
- debugging/diagnostics;
- native reference UI between CLI and final GUI clients.

Do not create a second wallet/service architecture for the Rust TUI. It should consume the same reusable wallet/SDK layers.

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

Validation workflows are PR/manual only so branch pushes and merges to `main` do not duplicate expensive builds. The lightweight `Main bundle` workflow is the deliberate `main`-push exception: it only creates/verifies `fresnica-main.bundle`, uploads it as an Actions artifact, and publishes a `main-bundle` commit status containing the artifact ID/run ID for automated discovery. The Mobile SDK release workflow remains the other exception: a release-marker change on `main` may publish the explicit release. Heavy Android/Apple packaging should be run only when the relevant Native SDK/platform boundary changes.

`sdk/compatibility/manifest.json` now records the compatible Core/SDK/Native/Mobile/WASM/React-Native version set. Run `node sdk/compatibility/validate.mjs` after changing any API/version constant or adapter contract; the matching GitHub check is lightweight and PR/manual-only.

## Smart-account conformance capture checkpoint

`providers/smart-account-kit` now has a real-Testnet auth-XDR recorder/verifier path. The browser smoke harness captures only the public relayer `func/auth` payload for a confirmed transfer, verifies the Protocol-27 signature payload/auth digest, extracts `context_rule_ids`, checks the WebAuthn challenge, and verifies the compact P-256 signature from the on-chain External signer key data. It then enables fixture download. The CLI verifier is `npm run fixture:verify -- <fixture.json>`.

Real-browser validation exposed upstream `smart-account-kit` 0.6.2 requesting WebAuthn `userVerification: "preferred"` while the deployed verifier rejects UV=0 (3117). Fresnica injects a required-UV WebAuthn adapter and the fixture verifier checks both UP and UV flags. The rerun succeeded on Testnet; its confirmed auth data is now the canonical `spec/test-vectors/smart-account-auth-v1.json` vector.

## Immediate Next Work

The next coherent implementation batches are:

1. **Keep Native SDK release explicit**: Apple Native SDK and the React Native 0.87 reference-consumer adapter path are validated, and marker-gated `native-sdk-v*` automation exists, but no release marker is present yet. Create one only when the version/artifact set is intentionally ready to publish; ordinary `main` pushes remain light.
2. **Keep the RN adapter contract stable**: Android and Apple have validated one-time build entry points plus compatibility manifests; normal Mobile builds should consume the stored binaries rather than rebuild adapter source.
3. **WASM/Web + smart account**: keep the real Testnet auth vector as the provider conformance baseline; implement a platform-native Mobile passkey provider only when Mobile integration starts rather than adding passkey-derived unlock-key APIs to WASM.
4. **Adapter extension contracts**: reserve Flutter/Desktop adapter interfaces without prematurely implementing every framework.
5. **Wallet fundamentals**: continue reusable wallet feature/SEP/hardware-signer work below product UI.
6. **Reference clients**: keep Rust CLI current; evaluate Rust TUI as an SDK/wallet engineering client.
7. **Product UX later**: once the above foundation is stable, concentrate effort on Mobile first, then Desktop/Web according to product priorities.

## Start Here Next Session

1. Verify `main` HEAD and relevant CI/release status.
2. Read `roadmap.md` for phase order.
3. Read `mobile-framework-adapter-contract.md` before changing Native SDK or RN packaging.
4. Read `mobile-app-migration-pr81-pr84.md` before starting the independent Mobile application.
5. Read `client-core-security.md` and `mobile-core-contract.md` before changing signer/passcode/system-auth boundaries.
6. Preserve Account != Signer, `C...` identity semantics, native-only routine signing and fresh-passcode-only Reveal/Export.
7. Treat `mobile-sdk-v0.1.0` as a transitional baseline, not the final multi-platform SDK layout.
8. Do not start product-specific Desktop/Web framework code until the universal SDK contract has a stable shape.

# Fresnica Roadmap

Updated: 2026-08-26

Fresnica now has a **multi-platform foundation with shared semantic contracts**. Rust Core is consumable through released native/WASM SDK boundaries, while Application Capabilities define wallet semantics above platform mechanisms and Application Flows own product-specific UI/UX.

The current architecture is:

```text
Application Flows
  -> Application Capabilities
  -> Fresnica SDK/Core + Stellar/network/repository/platform ports
```

The delivery chain for Core-owned security capabilities remains:

```text
Rust Core
  -> universal SDK contract
  -> platform SDK binaries / WASM package
  -> optional framework adapters
  -> platform Capability implementations
```

## Phase 0 - Architecture Foundation

Completed:

- Wallet / Account / Signer separation
- watch-only wallet concept
- Stellar SDK as protocol layer in the reference implementation
- WalletManager / application-capability / datastore layering
- CLI and TUI separation
- network-scoped chain-derived data
- full Stellar asset identity instead of code-only identity

## Phase 1 - Python Wallet Reference

Completed and retained as a behavior/reference implementation:

- CLI and Textual TUI
- wallet lifecycle management
- mnemonic / secret / watch-only wallets
- encrypted signing material
- balance, history and transfer flows
- SQLite chain-data cache
- contacts/address book
- assets/trustlines
- SDEX market/offer workflows
- anchor transfer workflows
- settings and local presentation state

The Python implementation remains useful for wallet behavior, UX experiments and conformance vectors. It is no longer the architectural center of the project.

## Phase 2 - Production Rust Core and CLI

Substantially completed:

- production Rust wallet derivation and signer primitives
- stable library-level `CoreClientApi` v3
- Account identity / Signer capability separation
- `G...` / `C...` identity parsing
- watch-only signer attachment identity verification
- protected software signer import/generation/sign/reveal lifecycle
- passcode re-protection without client-side secret disclosure
- `WalletUnlockKey` derivation/validation/signing boundary
- external Ed25519 prepare/apply signing boundary
- stable error classification
- thin process adapter for compatibility/reference use
- native Rust CLI direct SDK/Core reference client

The Rust CLI should remain a **reference native client** for proving that Core works without Mobile, JavaScript, Flutter or other application-framework dependencies.

## Phase 3 - Mobile Binding and Security Foundation

Completed foundation:

- FFI-neutral `fresnica-mobile-core` facade
- UniFFI 0.32.x Swift/Kotlin generation
- Android four-ABI Rust packaging
- Apple device/simulator Rust packaging and XCFramework generation
- Android Keystore / Apple Keychain `WalletUnlockKey` storage
- biometric/system-auth native signing orchestration
- React Native protected-signing bridge
- high-level React Native Core facade
- AccountRecord / SignerRecord / AccountSignerReference lifecycle model
- watch-only attach/detach semantics
- Realm-ready persistence reference
- staged/atomic app-passcode rotation
- protected account provisioning
- explicit signer Reveal / Export
- isolated CI for Rust/native/React Native/application-side changes
- Mobile SDK v0.1.0 integration release

`mobile-sdk-v0.1.0` is a **transitional integration release**. It proves the Core/native/security boundary but predates the finalized Native-SDK/framework-adapter packaging split.

## Phase 4 - Universal SDK Foundation - COMPLETED / MAINTENANCE

This foundation is complete enough for product consumers; future changes here are compatibility/release maintenance rather than a Mobile startup blocker.

### 4.1 Extract a platform-neutral SDK contract - COMPLETED

The platform-neutral `fresnica-sdk` contract is now the semantic owner above Rust Core, and the Mobile v0.1.0 facade is a compatibility wrapper over it.

Target layering:

```text
Rust Core
   |
   v
Stable SDK API / DTO / errors / conformance contract
   |
   +-- Native SDK packaging
   |     +-- Android
   |     +-- Apple
   |     +-- Windows
   |     +-- Linux
   |     +-- macOS
   |
   +-- WASM package
         +-- Web/browser consumers
```

The shared contract includes wallet/signing semantics, DTOs, errors, API versions and test vectors. Platform security/storage details do not belong in the Core contract.

### 4.2 Native SDK binaries - COMPLETED

The generalized `fresnica-native-sdk` UniFFI layer, Android AAR packaging and Apple Rust-FFI package are implemented. Android/Apple package paths also carry reusable native signer-authorization helpers. The Apple build additionally archives those SDK-owned Swift pieces into an importable `FresnicaSDK.xcframework`. On 2026-08-25 the complete `validate-apple-local.sh` flow passed on a real macOS/Xcode toolchain, including device/simulator Rust libraries, both XCFrameworks, generated Swift/security typechecking, and an independent consumer `import FresnicaSDK` check. Fresnica should publish **compiled platform SDK content**, not require application projects to rebuild Rust/UniFFI during ordinary builds.

Current native outputs:

```text
Android  -> complete direct-consumer AAR
Apple    -> validated FFI XCFramework + compiled `FresnicaSDK.xcframework` direct-consumer package
```

The Apple direct-consumer module is now validated on real macOS/Xcode. The React Native one-time Apple build path compiles only framework glue into a static `FresnicaRNAdapter.xcframework` against that validated module; it does not absorb SDK-owned Swift/security source. On 2026-08-25 the full path also passed on a macOS runner against a freshly generated React Native 0.87 project after real `pod install`, including device arm64 and simulator arm64/x86_64 adapter slices. Modern prebuilt React Native is consumed through CocoaPods' installed `React.xcframework`; Fresnica does not reconstruct React header namespaces from `node_modules`.

Desktop consumer surfaces are now defined in `docs/platforms/desktop/sdk-contract.md`:

```text
Rust desktop      -> consume `fresnica-sdk` directly
macOS Swift       -> same `FresnicaSDK` / `FresnicaSDKFFI` XCFrameworks as iOS
                    (implemented and validated on real macOS/Xcode, 2026-08-25)
Windows/Linux     -> select an explicit supported consumer language/framework before packaging
```

Android native applications may consume the AAR directly. The Apple iOS and macOS direct-consumer packaging paths have passed real Xcode validation. Fresnica will not publish a bare UniFFI `.dll`/`.so` as a language-neutral desktop SDK or treat UniFFI's internal C-compatible layer as a stable public C ABI.

Platform-specific signer authorization remains outside the pure Core contract, for example:

- Android Keystore / biometrics
- Apple Keychain / LocalAuthentication
- Windows DPAPI / Windows Hello integration where appropriate
- Linux Secret Service/libsecret integration where appropriate

### 4.3 WASM / Web - IMPLEMENTED / VALIDATED

The first browser boundary is implemented as `fresnica-wasm-sdk` over the universal SDK. It intentionally filters the native unlock-key surface rather than mirroring Mobile/Native APIs mechanically.

Browser routine software signing uses `signTransactionXdrWithPasscode(...)`: the fresh passcode enters Rust, Core derives/verifies the `WalletUnlockKey`, signs, and drops the key without returning it to JavaScript. `deriveUnlockKey`, `validateUnlockKey`, and raw unlock-key signing are not Web exports. External Ed25519 prepare/apply remains available.

The final WASM package, not `fresnica-core`/`fresnica-sdk`, opts into `getrandom`'s JavaScript/Web Crypto backend. A source-boundary test and generated-TypeScript-surface test enforce the filtered API.

Validated on macOS with the Rust + `wasm-bindgen` toolchain via `bindings/wasm/scripts/validate-local.sh`:

- `wasm32-unknown-unknown` target compile/check;
- release Web/WASM build and generated ES-module package;
- generated JS/TypeScript surface validation;
- Node-hosted runtime conformance against shared transaction vectors.

The passkey architecture is defined separately in `docs/capabilities/passkey-smart-account.md`: a passkey is a contract-account external signer, not a persistent browser `WalletUnlockKey`. The first interoperability target is Stellar's OpenZeppelin-based `smart-account-kit` model. A pinned provider boundary lives under `providers/smart-account-kit`, targeting upstream `smart-account-kit` 0.6.2 and the published 2026-07-09 Protocol 27 Testnet deployment. Its lifecycle/submission contract is mock-tested locally and deliberately delegates only the safe upstream `signAndSubmit` path. On 2026-08-25 the localhost browser harness completed a real WebAuthn/Testnet create/connect/sign-and-submit flow; the confirmed transaction's public relayer `func/auth` XDR was independently verified for the Protocol-27 digest, bound context-rule IDs, WebAuthn challenge, UP/UV flags and P-256 signature, then checked in as `spec/test-vectors/smart-account-auth-v1.json`. This remains a contract-account provider boundary, not an unlock-key API added to WASM.

Web should normally consume the WASM SDK directly. Add a web-framework adapter only if a framework creates a real integration need.

### 4.4 Framework adapters

Fresnica publishes **canonical adapter source/reference implementations** separately from Native SDK binaries.

Consumer rule:

```text
initial project integration
or framework/binding compatibility change
    -> compile adapter once in consumer environment
    -> store/version adapter binary + compatibility manifest

normal application builds
    -> link Native SDK binary
    -> link generated adapter binary
    -> do not rebuild Rust/Core
    -> do not rebuild adapter source
```

React Native is the first adapter target. Canonical Android/Apple adapter source targets `fresnica-native-sdk`; both platforms have one-time consumer build entry points plus compatibility manifest/rebuild checks. The Apple Native SDK XCFramework path, including the shared iOS + macOS slices, is validated on real macOS/Xcode, and the static React Native adapter XCFramework has passed a real React Native 0.87 CocoaPods reference-consumer build on macOS.

Planned adapter boundary:

- React Native: treat the validated Android/Apple one-time adapter builds as the canonical reference; application integration should pin the framework version and reuse the stored adapter binary + compatibility manifest
- Flutter Mobile/Desktop: reserve and document the same contract; implement when needed
- Desktop frameworks such as Electron/Node, Qt or .NET: reserve the extension boundary; implement based on product choice

See `docs/platforms/mobile/framework-adapter.md` for the current one-time adapter-build model. This contract should be generalized rather than replaced when Desktop/Flutter support is added.

### 4.5 Compatibility and release tooling

The universal SDK work should provide:

- SDK API versioning independent from framework versions
- machine-readable compatibility manifests
- canonical conformance vectors/tests
- native package release CI
- adapter build entry point/tooling
- clear rebuild-required diagnostics rather than silent adapter recompilation

The first repository-wide compatibility manifest now lives at `sdk/compatibility/manifest.json`. A lightweight Node validator checks the Core/SDK/Native/Mobile/WASM API constants, package versions, React Native adapter contract, and the pinned smart-account provider/upstream/Testnet fixture schema without invoking heavy platform builds. Its GitHub workflow is PR/manual-only.

The generalized Native SDK release contract is defined in `docs/sdk/native-release.md` with its own `native-sdk-v*` tag namespace and marker-gated workflow. `native-sdk-v0.1.0` established the generalized packaging boundary. The current Mobile security/HD baseline is `native-sdk-v0.2.1`: Native package 0.2.1, Native Binding API 2 over SDK API 3 / Core Client API 3, with React Native adapter source 0.2.0. v0.2 added Core-owned derivation of another explicit mnemonic index without mnemonic re-export and replaced per-signer biometric enrollment with one device System Auth Protection Domain plus no-biometric public-key wrapping for later signers. Android raw-AAR consumer validation, Apple iOS/macOS direct-consumer validation and React Native platform gates remain release requirements. The legacy `mobile-sdk-v0.1.0` publisher is retired from `main`; its tag/source remain historical migration material.

## Phase 5 - Application Capability Foundation and Standards - CURRENT / STABILIZING

The reusable wallet foundation is substantially implemented in the Rust reference clients. The current goal is to stabilize the cross-platform Application Capability contracts and let product-specific Application Flows evolve independently.

Current standards work:

- keep the five common architecture/security/platform contracts authoritative and small;
- maintain per-capability semantic contracts under `docs/capabilities/`;
- keep mature Account, Signer, Balance, Payment, Transaction, Trustline, SDEX, Anchor and Signing Coordination semantics Normative;
- keep Wallet, Backup / Restore, Asset Discovery / Catalog, History, Contacts, Application Security, Dapp, Ledger Authorization, External Signer and Network/Gateway Defined until stronger cross-platform implementation/protocol evidence justifies promotion;
- extract proven behavior from RefPython/Rust/native implementations as Reference Semantics instead of leaving it implicit in code;
- let Mobile/Web/Desktop implementations propose specification upgrades through evidence-backed documentation PRs instead of copying Rust internals.

The provider-neutral Core prepare/apply boundary is already sufficient for hardware wallets. Ledger is the first provider candidate, but its implementation is deliberately gated: Fresnica currently uses `stellar-xdr 28.0.0` while the current reviewed Stellar CLI workspace provides `stellar-ledger 27.1.0` on `stellar-xdr 27.0.0`, and Ledger has not yet published a Stellar-specific DMK signer kit. Do not add a lossy XDR-version conversion or move HID/BLE/WebHID into Core merely to close the checklist; see `docs/capabilities/external-signer.md`.

### SEP policy

Fresnica should follow Stellar ecosystem standards and official SDK behavior rather than inventing competing wallet-specific protocol semantics.

Current/reference anchor coverage includes SEP-1, Classic SEP-10, SEP-6, SEP-24 and the common SEP-12 customer status/update handoff. `fresnica-client` now owns the shared Rust anchor protocol boundary: issuer-bound discovery, verified two-phase direct-Classic SEP-10 challenge/session semantics, SEP-24-preferred / SEP-6-fallback transfer transport and transaction-status lookup. Exact-case asset matching, redirect-chain hardening and delegated/multisig SEP-10 remain known reference conformance gaps. The CLI keeps presentation, passcode prompting and reviewed withdrawal-payment confirmation outside that Capability boundary. SEP-45 contract-account execution remains separate. SEP-12/KYC has an explicit common customer status/update workflow; uncommon nested-value plus `/customer/files` handling remains intentionally deferred rather than faked or silently bypassed.

Future SEP adoption should be driven by wallet/product requirements and reviewed as protocol behavior below the UI layer.

## Phase 6 - Engineering Clients

### Rust CLI

The Rust CLI is already the primary lightweight native validation client. Keep it working as SDK/Core contracts evolve. Its SDK-boundary guard, Rust unit suite, release build and Python compatibility suite were all revalidated on GitHub Actions on 2026-08-25 after PR #95.

### Rust TUI - FUNCTIONAL REFERENCE CLIENT

The shared Rust application-client boundary is now explicit in `clients/rust-client` (`fresnica-client`). The Rust CLI consumes that layer for wallet storage/lifecycle, contacts, Horizon transport, account/balance/history reads, UI-free transaction orchestration and payment prepare/review/submit semantics. This keeps application orchestration reusable without moving persistence/network policy into the universal SDK or making the client layer a crypto authority.

The current `clients/rust-tui` reference client consumes the same client layer and provides:

- wallet identity/capability header;
- network-scoped session wallet switching;
- balances/liabilities;
- recent activity;
- manual refresh;
- reviewed XLM/issued-asset payment preparation;
- masked passcode entry and SDK-backed payment submission with shared pending-transaction protection;
- trustline add/limit/remove review and submission through shared Rust Capability implementations;
- SDEX BUY/SELL offer creation, offer update and offer cancellation through shared Rust SDEX Capability implementations;
- typed current open-offer display through the shared Rust SDEX Capability implementation;
- pair-scoped market view over shared order-book, recent-trade and candle semantics.

Its purpose remains SDK integration proving, wallet-flow experimentation, debugging/diagnostics, and a native reference UI between CLI and product GUI. Further terminal presentation can evolve without creating a second wallet semantic architecture or calling CLI command handlers from the TUI.

## Phase 7 - Product Wallet Experience

After the SDK and wallet behavior foundation are stable, shift the main effort to product experience.

### Mobile

The independent Mobile application owns:

- React Native product application
- Realm configuration/migrations and application persistence
- screens/navigation/state
- network/product orchestration
- wallet/account management UX
- one-time device System Auth Domain initialization, signer registration and recovery UX
- passcode rotation UX
- Reveal / Export UX
- hardware/external signer UX

Mobile consumes a pinned Fresnica Native SDK plus an adapter binary compiled once against its chosen React Native version.

The PR #81-#84 application-side donor/reference code remains migration material until the independent Mobile project has absorbed the required semantics.

### Desktop / Web

Desktop and Web product clients come after the shared SDK contract is proven. They should reuse the same wallet semantics rather than fork Mobile behavior.

Desktop consumes platform Native SDK binaries plus a framework adapter only when needed. Web consumes the WASM SDK and follows its separately reviewed browser security model.

## Immediate Next Work

1. **Finish documentation/contract stabilization**: keep `docs/README.md`, the five common contracts and `docs/capabilities/` as the authoritative cross-project vocabulary; remove remaining stale implementation-status claims rather than duplicating contracts in platform/state documents.
2. **Land and validate the current Rust reference batch**: after the documentation batch is pushed, run real Rust tests/release builds for the Anchor Capability extraction that follows the already-validated SEP-12 batch.
3. **Let independent Mobile integration proceed from the contracts**: Mobile Features implement Application Flows and may implement Capabilities with Stellar JS SDK + Native SDK + Mobile-owned repositories; do not require `fresnica-client` or mirror Rust module structure.
4. **Upgrade Defined capabilities only from concrete product evidence**: Backup/Restore, Ledger Authorization, Asset Discovery/Catalog, Dapp/session transport, History normalization, Contacts, Application Security, Wallet aggregate and Network/Gateway contracts should mature from real Mobile/Web/Desktop behavior.
5. **Keep SEP/hardware extensions demand-driven**: validate Anchor behavior against concrete anchors before nested `/customer/files`; keep SEP-45 separate; keep Ledger transport gated by exact XDR/provider compatibility.
6. **Preserve provider conformance baselines**: smart-account/passkey remains provider/Testnet reference material until a product needs a cross-platform capability contract.

## Architecture / Security Rules

Do not duplicate permanent architecture/security rules in the roadmap. The authoritative contracts are:

- [`architecture.md`](architecture.md);
- [`application-flows.md`](application-flows.md);
- [`application-capabilities.md`](application-capabilities.md);
- [`core-security-boundary.md`](core-security-boundary.md);
- [`platform-implementation.md`](platform-implementation.md).

Roadmap changes must conform to those documents. If a stable rule changes, update the relevant contract first and then adjust roadmap state.

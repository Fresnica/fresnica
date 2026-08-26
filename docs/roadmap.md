# Fresnica Roadmap

Updated: 2026-08-26

Fresnica now has an **SDK-first, multi-platform foundation**. Rust Core is consumable through released native/WASM SDK boundaries, while React Native and future frameworks remain thin adapters. The independent Mobile product can start against the pinned Native SDK without waiting for more Core infrastructure.

The product sequence is deliberately:

```text
Core semantics
  -> universal SDK contract
  -> platform SDK binaries / WASM package
  -> framework adapter source + one-time consumer build
  -> wallet functional coverage / SEP alignment
  -> product-specific wallet experience
```

## Phase 0 - Architecture Foundation

Completed:

- Wallet / Account / Signer separation
- watch-only wallet concept
- Stellar SDK as protocol layer in the reference implementation
- WalletManager / service / datastore layering
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
- stable library-level `CoreClientApi` v2
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

Desktop consumer surfaces are now defined in `desktop-sdk-contract.md`:

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

The passkey architecture is defined separately in `passkey-smart-account.md`: a passkey is a contract-account external signer, not a persistent browser `WalletUnlockKey`. The first interoperability target is Stellar's OpenZeppelin-based `smart-account-kit` model. A pinned provider boundary lives under `providers/smart-account-kit`, targeting upstream `smart-account-kit` 0.6.2 and the published 2026-07-09 Protocol 27 Testnet deployment. Its lifecycle/submission contract is mock-tested locally and deliberately delegates only the safe upstream `signAndSubmit` path. On 2026-08-25 the localhost browser harness completed a real WebAuthn/Testnet create/connect/sign-and-submit flow; the confirmed transaction's public relayer `func/auth` XDR was independently verified for the Protocol-27 digest, bound context-rule IDs, WebAuthn challenge, UP/UV flags and P-256 signature, then checked in as `spec/test-vectors/smart-account-auth-v1.json`. This remains a contract-account provider boundary, not an unlock-key API added to WASM.

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

See `mobile-framework-adapter-contract.md` for the current one-time adapter-build model. This contract should be generalized rather than replaced when Desktop/Flutter support is added.

### 4.5 Compatibility and release tooling

The universal SDK work should provide:

- SDK API versioning independent from framework versions
- machine-readable compatibility manifests
- canonical conformance vectors/tests
- native package release CI
- adapter build entry point/tooling
- clear rebuild-required diagnostics rather than silent adapter recompilation

The first repository-wide compatibility manifest now lives at `sdk/compatibility/manifest.json`. A lightweight Node validator checks the Core/SDK/Native/Mobile/WASM API constants, package versions, React Native adapter contract, and the pinned smart-account provider/upstream/Testnet fixture schema without invoking heavy platform builds. Its GitHub workflow is PR/manual-only.

The generalized Native SDK release contract is defined in `native-sdk-release.md` with its own `native-sdk-v*` tag namespace and marker-gated workflow. `native-sdk-v0.1.0` established the generalized packaging boundary. The current Mobile security/HD baseline is `native-sdk-v0.2.1`: Native package 0.2.1, Native Binding API 2 over SDK API 3 / Core Client API 3, with React Native adapter source 0.2.0. v0.2 added Core-owned derivation of another explicit mnemonic index without mnemonic re-export and replaced per-signer biometric enrollment with one device System Auth Protection Domain plus no-biometric public-key wrapping for later signers. Android raw-AAR consumer validation, Apple iOS/macOS direct-consumer validation and React Native platform gates remain release requirements. The legacy `mobile-sdk-v0.1.0` publisher is retired from `main`; its tag/source remain historical migration material.

## Phase 5 - Wallet Functional Foundation and Standards - CURRENT

Continue wallet functionality while the SDK foundation is being completed. The goal is to stabilize reusable wallet behavior before investing heavily in final product UX.

Priority areas:

- account / signer / watch-only lifecycle
- balance and portfolio behavior
- send/payment and transaction review
- assets/trustlines
- history/activity
- SDEX offers/markets
- contacts/address resolution
- network configuration
- hardware/external signer transport

The provider-neutral Core prepare/apply boundary is already sufficient for hardware wallets. Ledger is the first provider candidate, but its implementation is deliberately gated: Fresnica currently uses `stellar-xdr 28.0.0` while the current reviewed Stellar CLI workspace provides `stellar-ledger 27.1.0` on `stellar-xdr 27.0.0`, and Ledger has not yet published a Stellar-specific DMK signer kit. Do not add a lossy XDR-version conversion or move HID/BLE/WebHID into Core merely to close the checklist; see `hardware-signer.md`.
- anchor workflows

### SEP policy

Fresnica should follow Stellar ecosystem standards and official SDK behavior rather than inventing competing wallet-specific protocol semantics.

Current/reference anchor coverage includes SEP-1, SEP-10, SEP-6 and SEP-24. The Rust reference client now mirrors the Python SEP-1 + SEP-6/SEP-24 capability-discovery path (`anchor discover CODE:GISSUER`); authenticated transfer execution remains in the Python reference until the Rust path grows an explicit SEP-10/SEP-45 authorization design. SEP-12/KYC remains a future explicit workflow rather than something to fake or silently bypass.

Future SEP adoption should be driven by wallet/product requirements and reviewed as protocol behavior below the UI layer.

## Phase 6 - Engineering Clients

### Rust CLI

The Rust CLI is already the primary lightweight native validation client. Keep it working as SDK/Core contracts evolve. Its SDK-boundary guard, Rust unit suite, release build and Python compatibility suite were all revalidated on GitHub Actions on 2026-08-25 after PR #95.

### Rust TUI - STARTED

The shared Rust application-client boundary is now explicit in `clients/rust-client` (`fresnica-client`). The Rust CLI consumes that layer for wallet storage/lifecycle, contacts, Horizon transport and account/balance/history reads. This keeps application orchestration reusable without moving persistence/network policy into the universal SDK.

The first `clients/rust-tui` slice consumes the same client layer and provides:

- wallet identity/capability header;
- network-scoped session wallet switching;
- balances/liabilities;
- recent activity;
- manual refresh.

Its purpose remains SDK integration proving, wallet-flow experimentation, debugging/diagnostics, and a native reference UI between CLI and product GUI. Write flows should be exposed through reusable client services before being added to the TUI; do not create a second wallet/service architecture or call CLI command handlers from the TUI.

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

1. **Start independent Mobile integration now** from `mobile-sdk-usage.md`: pin `native-sdk-v0.2.1` / Native Binding API 2 and an exact React Native version, build Android/Apple RN adapter binaries once, store their compatibility manifest, and prove `FresnicaCore.parseAccount` on both platforms. Establish one app passcode and optionally initialize one device System Auth Domain; later signers register with passcode verification but no repeat biometric prompt.
2. **Move Mobile application ownership out of this repository**: Realm schema/migrations, Account/Signer persistence, watch-only/import/generate/passcode/reveal UX and product state belong in `fresnica-mobile`; preserve the #81-#84 invariants from `mobile-app-migration-pr81-pr84.md`.
3. **Keep the SDK boundary stable**: new Core/SDK capability work may continue independently, but Mobile should upgrade through pinned Native SDK releases and `NATIVE_BINDING_API_VERSION`, not by compiling Rust/Core in normal app builds.
4. Continue reusable wallet/SEP work (next anchor gap: SEP-12 customer-information handoff) below product UI.
5. Keep the real smart-account Testnet vector as provider conformance baseline; add a platform-native Mobile passkey provider only when Mobile product integration reaches contract-account signing.
6. Keep hardware/external signer transport gated by the reviewed Ledger/XDR compatibility constraints rather than forcing lossy conversion.
7. Keep Rust CLI as the reference native engineering client; Desktop/Web product work follows the same stable SDK boundaries rather than creating new Core architectures.

## Non-negotiable Architecture Rules

- Account != Signer != Recovery Source; the relationships are not necessarily one-to-one.
- `C...` account identity is not an Ed25519 public key.
- Core/SDK owns cryptographic signer semantics; clients own persistence, network state and product orchestration.
- Framework adapters are mechanical integration glue, not security authorities.
- Routine application builds must not rebuild Rust/Core or framework adapter source.
- Plaintext secret/mnemonic exposure remains exceptional and explicit.
- `WalletUnlockKey` and equivalent native authorization material must not cross into JavaScript/Dart merely for convenience.
- System auth is lower privilege than the Fresnica app passcode: it may authorize routine signing, but not Reveal/Export, passcode change, or recovery-root replacement.
- Global passcode rotation stages `reprotect` for every protected software signer, atomically commits the complete envelope set, then replaces wrapped unlock-key records in the existing System Auth Domain.
- UniFFI's internal C-compatible layer is not a stable Fresnica public C ABI.
- Full Stellar asset identity and network scoping remain authoritative.
- Stellar protocol/SEP behavior should reuse official primitives and standards wherever practical.

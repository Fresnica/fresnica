# Fresnica Roadmap

Updated: 2026-08-25

Fresnica is moving from a Mobile-specific integration phase to an **SDK-first, multi-platform foundation**. The immediate goal is to make the Rust Core consumable through stable platform SDKs, then keep React Native, Flutter, desktop frameworks and future clients as thin framework adapters over that SDK boundary.

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
- native Rust CLI direct-link client

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

## Phase 4 - Universal SDK Foundation - CURRENT

This is the current priority.

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

### 4.2 Native SDK binaries - IN PROGRESS

The generalized `fresnica-native-sdk` UniFFI layer, Android AAR packaging and Apple Rust-FFI package are implemented. Android/Apple package paths also carry reusable native signer-authorization helpers. The Apple build now additionally archives those SDK-owned Swift pieces into an importable `FresnicaSDK.xcframework`; that new packaging step still needs a real macOS/Xcode validation run before 4.2 is called complete. Fresnica should publish **compiled platform SDK content**, not require application projects to rebuild Rust/UniFFI during ordinary builds.

Current native outputs:

```text
Android  -> complete direct-consumer AAR
Apple    -> FFI XCFramework + compiled `FresnicaSDK.xcframework` build path (macOS validation pending)
```

The Apple direct-consumer module build path is implemented, but the resulting `FresnicaSDK.xcframework` still needs real macOS/Xcode validation before the Native SDK binary boundary is called complete. The React Native one-time Apple build path now compiles only framework glue into a static `FresnicaRNAdapter.xcframework` against that module; it does not absorb SDK-owned Swift/security source. Both new Apple XCFramework paths still require a real Xcode validation run.

Desktop consumer surfaces are now defined in `desktop-sdk-contract.md`:

```text
Rust desktop      -> consume `fresnica-sdk` directly
macOS Swift       -> extend compiled `FresnicaSDK` after current Apple validation
Windows/Linux     -> select an explicit supported consumer language/framework before packaging
```

Android native applications may consume the AAR directly. The Apple direct-consumer packaging path is implemented and awaits real Xcode validation. Fresnica will not publish a bare UniFFI `.dll`/`.so` as a language-neutral desktop SDK or treat UniFFI's internal C-compatible layer as a stable public C ABI.

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

The passkey architecture is now defined separately in `passkey-smart-account.md`: a passkey is a contract-account external signer, not a persistent browser `WalletUnlockKey`. The first interoperability target is Stellar's OpenZeppelin-based `smart-account-kit` model. A pinned provider boundary now lives under `providers/smart-account-kit`, targeting upstream `smart-account-kit` 0.6.2 and the published 2026-07-09 Protocol 27 Testnet deployment. Its lifecycle/submission contract is mock-tested locally, deliberately delegates only the safe upstream `signAndSubmit` path, and includes a localhost browser smoke harness for create/fund/discover/native-XLM transfer. The next checkpoint is executing that harness with real WebAuthn/Testnet, followed by auth-XDR/context-rule fixtures; this is not an unlock-key API added to WASM.

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

React Native is the first adapter target. Canonical Android/Apple adapter source now targets `fresnica-native-sdk`; both platforms have one-time consumer build entry points plus compatibility manifest/rebuild checks. The new Apple Native SDK and adapter XCFramework paths remain pending real macOS/Xcode validation as described in 4.2.

Planned adapter boundary:

- React Native: validate the Apple Native SDK + adapter XCFramework path on macOS/Xcode, then treat Android/Apple one-time adapter builds as the canonical reference
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

The first repository-wide compatibility manifest now lives at `sdk/compatibility/manifest.json`. A lightweight Node validator checks the Core/SDK/Native/Mobile/WASM API constants, package versions, and React Native adapter contract without invoking heavy platform builds. Its GitHub workflow is PR/manual-only.

Native SDK release automation is intentionally not enabled yet: the Apple direct-consumer binary package must pass the real macOS/Xcode validation gate first. The existing `mobile-sdk-v0.1.0` release workflow remains a transitional compatibility release path, not the template for the generalized Native SDK.

## Phase 5 - Wallet Functional Foundation and Standards

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

The provider-neutral Core prepare/apply boundary is already sufficient for hardware wallets. Ledger is the first provider candidate, but its implementation is deliberately gated: Fresnica currently uses `stellar-xdr 28.0.0` while the latest reviewed `stellar-ledger` package is still on XDR 27, and Ledger has not yet published a Stellar-specific DMK signer kit. Do not add a lossy XDR-version conversion or move HID/BLE/WebHID into Core merely to close the checklist; see `hardware-signer.md`.
- anchor workflows

### SEP policy

Fresnica should follow Stellar ecosystem standards and official SDK behavior rather than inventing competing wallet-specific protocol semantics.

Current/reference anchor coverage includes SEP-1, SEP-10, SEP-6 and SEP-24. SEP-12/KYC remains a future explicit workflow rather than something to fake or silently bypass.

Future SEP adoption should be driven by wallet/product requirements and reviewed as protocol behavior below the UI layer.

## Phase 6 - Engineering Clients

### Rust CLI

The Rust CLI is already the primary lightweight native validation client. Keep it working as SDK/Core contracts evolve.

### Rust TUI

A Rust TUI is worth considering after the universal SDK boundary is clear.

Its purpose is primarily:

- SDK integration proving ground
- wallet-flow playground
- debugging/diagnostic client
- native reference implementation between CLI and product GUI

It should reuse SDK/wallet services and must not become a second independent wallet architecture.

## Phase 7 - Product Wallet Experience

After the SDK and wallet behavior foundation are stable, shift the main effort to product experience.

### Mobile

The independent Mobile application owns:

- React Native product application
- Realm configuration/migrations and application persistence
- screens/navigation/state
- network/product orchestration
- wallet/account management UX
- system-auth enrollment/recovery UX
- passcode rotation UX
- Reveal / Export UX
- hardware/external signer UX

Mobile consumes a pinned Fresnica Native SDK plus an adapter binary compiled once against its chosen React Native version.

The PR #81-#84 application-side donor/reference code remains migration material until the independent Mobile project has absorbed the required semantics.

### Desktop / Web

Desktop and Web product clients come after the shared SDK contract is proven. They should reuse the same wallet semantics rather than fork Mobile behavior.

Desktop consumes platform Native SDK binaries plus a framework adapter only when needed. Web consumes the WASM SDK and follows its separately reviewed browser security model.

## Immediate Next Work

1. Validate `FresnicaSDK.xcframework` and the Apple React Native adapter on a real macOS/Xcode toolchain; only after that validation add Native SDK release automation.
2. Run the pinned `smart-account-kit` provider through a real browser/WebAuthn + Protocol 27 Testnet create/connect/sign-and-submit flow, then capture auth-XDR/context-rule conformance fixtures.
3. Keep `sdk/compatibility/manifest.json` green as API/package versions change; do not add another parallel version source.
4. Extend the validated Apple Native SDK package to macOS Swift after the iOS package is proven. Keep Windows/Linux non-Rust packaging deferred until a concrete consumer language/framework is selected.
5. Continue Phase 5 wallet fundamentals below product UI, with hardware/external signer transport as the next signer-capability gap and existing SEP-aligned behavior reused rather than reimplemented.
6. Keep the Rust CLI as the reference native client and use the Rust TUI only as an engineering client when it materially improves SDK/wallet-flow validation.
7. Move concentrated effort into Mobile/Desktop/Web product UX only after these remaining platform/signing validation checkpoints are stable.

## Non-negotiable Architecture Rules

- Account is not Signer; the relationship is not necessarily one-to-one.
- `C...` account identity is not an Ed25519 public key.
- Core/SDK owns cryptographic signer semantics; clients own persistence, network state and product orchestration.
- Framework adapters are mechanical integration glue, not security authorities.
- Routine application builds must not rebuild Rust/Core or framework adapter source.
- Plaintext secret/mnemonic exposure remains exceptional and explicit.
- `WalletUnlockKey` and equivalent native authorization material must not cross into JavaScript/Dart merely for convenience.
- Full Stellar asset identity and network scoping remain authoritative.
- Stellar protocol/SEP behavior should reuse official primitives and standards wherever practical.

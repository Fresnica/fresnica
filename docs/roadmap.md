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

### 4.1 Extract a platform-neutral SDK contract

Refactor the current Mobile-oriented facade into a **general SDK surface** suitable for Mobile, Desktop and other native consumers without duplicating Core behavior.

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

### 4.2 Native SDK binaries

Fresnica should publish **compiled platform SDK content**, not require application projects to rebuild Rust/UniFFI during ordinary builds.

Target outputs include:

```text
Android  -> compiled AAR
Apple    -> compiled XCFramework/package
Windows  -> compiled native library/package
Linux    -> compiled native library/package
macOS    -> compiled framework/native library/package
```

A native application may consume the platform SDK directly with no framework adapter.

Platform-specific signer authorization remains outside the pure Core contract, for example:

- Android Keystore / biometrics
- Apple Keychain / LocalAuthentication
- Windows DPAPI / Windows Hello integration where appropriate
- Linux Secret Service/libsecret integration where appropriate

### 4.3 WASM / Web

Add a WASM-facing SDK path that preserves the same Core semantics where technically appropriate.

Do **not** assume the Mobile `WalletUnlockKey`/Keychain model can be copied into browsers. Web key protection, browser storage and authorization require a separate reviewed security design.

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

React Native is the first fully implemented adapter target.

Planned adapter boundary:

- React Native: implement canonical source + build tooling now
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

1. Generalize the current Mobile-facing facade into a universal SDK contract without changing established Core security semantics.
2. Separate framework-specific code from future Native SDK release binaries; retain `v0.1.0` only as the transitional baseline.
3. Make React Native the first canonical one-time-build adapter implementation, with generated binary + compatibility manifest tooling.
4. Define Desktop Native SDK targets and WASM/Web contract boundaries; reserve Flutter/Desktop adapter interfaces without prematurely implementing every framework.
5. Continue wallet functional coverage and SEP-aligned behavior, including hardware/external signer work when appropriate.
6. Keep the Rust CLI as the reference native client and evaluate a Rust TUI as an engineering client.
7. Once these foundations are stable, move concentrated effort into Mobile/Desktop/Web wallet experience.

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

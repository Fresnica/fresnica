# React Native Upgrade Playbook

Status: **required mobile maintenance procedure**.

This document defines how Fresnica upgrades React Native without allowing framework churn to leak into wallet cryptography, signer semantics, persistence semantics, or native authorization policy.

The rule is simple:

> A React Native upgrade may replace host glue and build integration. It must not redefine Fresnica Core, Account/Signer semantics, protected signer envelopes, `WalletUnlockKey`, or signing authorization.

## Current baseline

As of 2026-08-25, React Native 0.87 is the latest stable release. It makes the Strict TypeScript API the default, requires Node.js 22.13+, supports Android Gradle Plugin 9, requires Kotlin 2.0+, raises Android compile requirements, and adds experimental Swift Package Manager support.

React Native's New Architecture is the long-term direction. Fresnica's current conventional native-module bridge is therefore treated as replaceable host glue, not as part of the wallet security architecture.

Do not encode assumptions from one React Native release into Core or the mobile Rust facade.

## Layer ownership

An upgrade must preserve these boundaries:

```text
React Native UI / navigation / product state        replaceable
        |
RN module registration / Codegen / TurboModule      replaceable
        |
Fresnica Native SDK adapter                     mostly stable
        |
Keychain / Keystore authorization layer           stable security boundary
        |
UniFFI Swift / Kotlin API                            generated boundary
        |
fresnica-mobile-core                                stable facade
        |
CoreClientApi                                        stable contract
        |
Fresnica Core                                        cryptographic authority
```

The Account/Signer client layer is separate from the React Native bridge:

```text
UI
 |
WalletLifecycle / provisioning / export coordinators
 |
WalletStore
 |
Realm adapter
```

Changing React Native does not justify changing this model.

## Expected upgrade work

The following areas may legitimately change during a React Native upgrade:

- Android and iOS host project files;
- Node / Metro / Gradle / AGP / Kotlin / Xcode / CocoaPods or SwiftPM versions;
- native-module registration;
- `ReactContextBaseJavaModule` / `ReactPackage` glue;
- `RCT_EXTERN_MODULE` glue;
- TurboModule / Codegen specs and generated React Native bindings;
- Objective-C++ compatibility glue required by React Native's C++ runtime;
- React Native-facing TypeScript types and imports;
- host lifecycle integration required by a newer React Native release.

These changes should remain thin adapters around existing Fresnica-owned SDK/Capability boundaries.

## Work that must not be redone merely because React Native changed

Do not rewrite or duplicate any of the following during an RN upgrade unless an independent Fresnica requirement demands it:

- Rust Core cryptography;
- `CoreClientApi` semantics;
- mobile FFI DTO/error semantics;
- protected signer envelope format;
- signer identity verification;
- transaction hashing or signature validation;
- `WalletUnlockKey` derivation or validation;
- Keychain/Keystore authorization semantics;
- AccountRecord / SignerRecord / AccountSignerReference semantics;
- watch-only upgrade/downgrade rules;
- Realm persistence graph;
- staged/atomic passcode rotation;
- explicit Reveal / Export security policy;
- external Ed25519 prepare/apply signing semantics.

If an RN upgrade appears to require any of these to change, stop and treat it as an architecture regression until proven otherwise.

## Upgrade procedure

### 1. Establish the target before touching Fresnica code

Record:

- current React Native version;
- target React Native version;
- required Node version;
- Android Gradle Plugin, Gradle, Kotlin, compileSdk and NDK requirements;
- iOS deployment/Xcode/CocoaPods/SwiftPM requirements;
- New Architecture requirements and removed legacy APIs;
- Metro/TypeScript API changes;
- relevant React Native Upgrade Helper diff.

Prefer official React Native release notes, upgrade documentation, and Upgrade Helper over third-party migration posts.

### 2. Upgrade a minimal host shell first

Before changing wallet logic, prove that an empty/minimal Fresnica host can:

- start React Native on Android;
- start React Native on iOS;
- load a trivial native module;
- build release variants on both platforms.

This separates framework/build failures from wallet failures.

### 3. Reattach the existing Fresnica Native SDK adapter

Do not port Core logic into the new RN mechanism.

The new bridge must call the existing platform SDK/authorization surface. If migrating to TurboModules, implement only the RN-specific spec/registration/glue necessary to expose the same high-level operations.

Routine signing must remain:

```text
JS request
  -> native authorization layer
  -> Keychain/Keystore user authentication
  -> WalletUnlockKey in native memory
  -> UniFFI
  -> Rust Core
```

`WalletUnlockKey`, biometric crypto objects, and native signing sessions must never become JavaScript values as part of an upgrade.

### 4. Preserve the JavaScript-facing contract where possible

Prefer keeping Fresnica's high-level method names and result shapes stable while replacing their RN implementation underneath.

Examples:

- `parseAccount`
- `protectSecret`
- `protectMnemonic`
- `generateMnemonic`
- `reprotect`
- `reveal`
- `prepareEd25519Signing`
- `applyEd25519Signature`
- `signWithSystemAuth`
- `signWithPasscode`

Do not expose raw `deriveUnlockKey`, `validateUnlockKey`, or raw `signTransactionXdr` to JavaScript to make a migration easier.

### 5. Migrate platform glue independently

#### Android

Expected migration surface:

- React Native module base classes/registration;
- Codegen/TurboModule integration;
- Gradle/AGP/Kotlin configuration;
- React Native package namespace/import changes;
- Activity/lifecycle APIs required by biometric prompts.

Keep `FresnicaSignerAuthorization`, Keystore handling, generated UniFFI API usage, and Rust packaging independent from the RN module class.

#### Apple

Expected migration surface:

- module registration;
- TurboModule/Codegen spec;
- Objective-C/Objective-C++ glue required by React Native;
- Xcode/CocoaPods/SwiftPM integration.

Keep Swift signer authorization, Keychain handling, generated UniFFI API usage, and Fresnica Capability/SDK semantics independent from RN registration glue.

React Native's Swift New Architecture guidance still requires small Objective-C++ glue because the React Native core is C++-heavy. Keep that glue minimal and outside wallet/security logic.

### 6. Verify security invariants before product UI

Before reconnecting the full application, verify:

- routine software signing never returns unlock-key bytes to JS;
- biometric/system authentication authorizes the exact native signing operation;
- explicit Reveal / Export still requires a fresh app passcode;
- generated mnemonic/secret plaintext is not persisted in Realm/navigation/global state;
- watch-only accounts require no signer material;
- Account and Signer remain separate records;
- a delegated/multisig signer may differ from the account address;
- contract `C...` identity is not converted into an Ed25519 master-key assumption;
- passcode rotation still commits all new envelopes atomically before system-auth cleanup.

### 7. Reconnect product flows

Only after native/security verification passes, reconnect:

- account list/switching;
- create/import/generate;
- watch-only import/upgrade/downgrade;
- send/sign flows;
- settings/passcode rotation;
- Reveal / Export;
- hardware/external signer flows.

Product-state bugs at this stage must not be fixed by weakening the native security boundary.

## CI matrix

Run only the gates implied by the changed layer.

| Changed area | Required CI |
| --- | --- |
| Mobile application lifecycle/persistence/product coordinator | Mobile-owned TypeScript + RN lifecycle tests |
| RN host glue under Android only | Android native-module/AAR compile + lint |
| RN host glue under Apple only | Apple native-module type-check/build |
| Keychain/Keystore authorization code | corresponding platform auth/native signing gate |
| `bindings/native` / adapter API metadata | corresponding Native SDK + adapter conformance |
| Rust Core / `CoreClientApi` | Rust Core + SDK + active binding conformance |
| Android/Apple Rust packaging scripts or ABI configuration | platform packaging gates |
| docs only | no mobile build |

During an actual React Native version jump, run one explicit **full host integration gate** after the layer-specific work is green. This is different from running four-ABI binding builds on every Realm/TypeScript edit.

## Upgrade acceptance criteria

An RN upgrade is complete only when:

1. Android debug/release host builds pass.
2. iOS simulator/device host builds pass.
3. Fresnica RN API shape is unchanged or intentionally versioned.
4. native system-auth signing passes on both platforms.
5. create/import/generate/watch-only/reprotect/export lifecycle tests pass.
6. no plaintext signer material is newly persisted or logged.
7. no unlock-key API is exposed to JavaScript.
8. Account/Signer persistence migrations are either unnecessary or explicitly reviewed.
9. Core/binding test vectors remain unchanged unless there is a separately approved Core change.

## Rollback criteria

Do not continue an RN upgrade by weakening Fresnica boundaries. Roll back or isolate the upgrade if it would require:

- storing private keys/mnemonics in JS state to work around bridge problems;
- exposing `WalletUnlockKey` to JS;
- duplicating Core crypto in Swift/Kotlin/TypeScript;
- collapsing Account and Signer back into one object;
- changing protected-envelope format only for bridge convenience;
- bypassing Core signer/signature verification;
- replacing atomic passcode rotation with per-wallet best-effort writes.

## When to reconsider UniFFI or direct JSI

A React Native upgrade alone is not a reason to replace UniFFI.

Reconsider the binding architecture only if there is measured evidence that the existing Swift/Kotlin-to-UniFFI boundary causes a material product problem, or if the supported UniFFI backend becomes incompatible with required platform toolchains.

Likewise, direct Rust-to-JSI should be considered only for a demonstrated high-frequency performance requirement. Fresnica's current cryptographic operations are low-frequency control operations where keeping native authorization outside JavaScript is more important than minimizing a small amount of bridge overhead.

## References

Use the official React Native release notes and New Architecture documentation for the target version. At the time this playbook was written, the current reference points were React Native 0.87 release notes and the official Swift TurboModule guidance.

# Fresnica Desktop SDK Contract

Updated: 2026-08-25

## Purpose

Desktop support reuses the same `fresnica-sdk` semantic contract as Mobile and Web, but Fresnica does **not** pretend that one generated `.dll`/`.so` is a language-neutral public SDK.

The direct-consumer language surface must be explicit before a desktop package is called supported.

## Decision

The desktop baseline is split by consumer language rather than by operating system alone:

```text
Rust desktop client
  -> consume `fresnica-sdk` directly

macOS Swift client
  -> consume compiled `FresnicaSDK` Swift module/package
  -> same XCFramework product as iOS, with universal macOS slices
  -> real-Xcode validation passed on 2026-08-25

Windows/Linux non-Rust client
  -> choose a supported consumer language/framework first
  -> then publish the matching binding/package
  -> do not expose UniFFI internals as an accidental public C ABI
```

This keeps Core semantics shared while allowing packaging/security integration to remain platform-specific.

## Rust desktop

Rust is already a first-class direct-consumer surface through `sdk/rust`.

Use cases include:

- `clients/rust-cli`;
- a future Rust TUI;
- native Rust desktop applications;
- engineering/conformance clients.

Rust consumers link the crate API directly. Fresnica does not promise a stable Rust compiler ABI or distribute a raw `.rlib` as a cross-toolchain binary SDK contract.

## macOS Swift

Swift is an official UniFFI target and matches the existing Apple Native SDK model.

Target shape:

```text
FresnicaSDK.xcframework
  - iOS device
  - iOS simulator
  - macOS universal framework

FresnicaSDKFFI.xcframework
  - matching iOS and universal macOS low-level Rust FFI slices
```

The public consumer imports `FresnicaSDK`; generated FFI declarations remain an implementation dependency of that module.

The macOS extension reuses the existing Swift DTO/error surface and does not fork wallet/signing semantics. The packaging implementation now builds `aarch64-apple-darwin` and `x86_64-apple-darwin`, combines them into the Apple XCFrameworks, and opts Keychain operations into the macOS Data Protection Keychain. The expanded `validate-apple-local.sh` passed on a real Xcode toolchain on 2026-08-25, validating the shared iOS + universal macOS Apple package path.

## Windows and Linux

UniFFI's built-in first-class foreign-language targets are Kotlin, Swift and Python, with Ruby maintained separately. It does not provide an official generic .NET/C#/Dart/Node desktop surface.

Therefore Fresnica does not currently declare a generic Windows or Linux binary SDK for arbitrary languages.

A supported Windows/Linux package requires an explicit product consumer, for example:

- Rust: use `fresnica-sdk` directly;
- Python: an official UniFFI-generated Python package may be produced if a Python binary SDK is actually required;
- Kotlin/JVM: may be evaluated if a JVM desktop product is selected;
- .NET/C#, Qt/C++, Electron/Node, Flutter/Dart: define and review the adapter/binding strategy when that product is selected.

Third-party UniFFI language generators may be evaluated, but they are not part of the stable Fresnica SDK contract merely because they exist.

## No accidental public C ABI

The C-compatible symbols and headers used internally by UniFFI are implementation details.

Do not:

- document generated UniFFI C symbols as Fresnica's stable C API;
- publish a `.dll`/`.so` alone and call it a complete desktop SDK;
- hand-write a parallel C ABI only to make the platform matrix look complete;
- let a framework adapter recreate cryptography, protected-envelope parsing, signer identity verification or error semantics.

A future stable C ABI is possible, but it requires its own versioned contract, ownership rules and conformance suite.

## Desktop security boundary

The universal SDK remains the cryptographic/signing semantic authority. Desktop platform code owns secure credential release and application session policy.

Potential platform integrations are separate from Core:

- macOS: Keychain / LocalAuthentication;
- Windows: DPAPI / Windows Hello where appropriate;
- Linux: Secret Service / libsecret where appropriate.

The same invariants apply:

- `WalletUnlockKey` is routine native authorization material and must remain outside framework scripting layers;
- Reveal/Export requires a fresh Fresnica app passcode;
- protected signer envelopes remain opaque;
- Account identity and Signer capability remain separate;
- external Ed25519 signing continues to use prepare/apply.

## Framework adapters

Desktop frameworks are adapters over a supported direct-consumer SDK surface, not new security authorities.

Examples:

```text
Electron / Node
Flutter Desktop
Qt
.NET
```

Implement an adapter only when the corresponding product/framework is selected. The adapter should follow the same one-time-build and compatibility-manifest model already established for React Native where that model fits the framework.

## Release criteria

A desktop target is considered supported only when all of the following exist:

1. an explicit consumer language/API surface;
2. a reproducible package artifact or documented source-consumption contract;
3. SDK/API version compatibility metadata;
4. conformance tests against shared Fresnica vectors;
5. platform security/storage ownership documented;
6. a direct-consumer smoke test that does not rebuild or reimplement Core semantics unexpectedly.

Until those criteria are met, the target is reserved rather than advertised as a completed SDK.

# Mobile Core Platform Packaging

This document describes the reproducible outputs produced from `fresnica-mobile-core` before the React Native adapter is added.

## Android

The Xaman-derived Android application currently targets API 26+ and builds four ABIs. Fresnica keeps that compatibility baseline for the first integration:

| Android ABI | Rust target |
| --- | --- |
| `armeabi-v7a` | `armv7-linux-androideabi` |
| `x86` | `i686-linux-android` |
| `x86_64` | `x86_64-linux-android` |
| `arm64-v8a` | `aarch64-linux-android` |

The packaging script requires the Android NDK and `cargo-ndk`:

```sh
cargo install cargo-ndk --version 4.1.2 --locked
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/26.1.10909125"
bash bindings/mobile/scripts/build-android.sh
```

Default output:

```text
bindings/mobile/build/android/
  jniLibs/
    armeabi-v7a/libfresnica_mobile_core.so
    x86/libfresnica_mobile_core.so
    x86_64/libfresnica_mobile_core.so
    arm64-v8a/libfresnica_mobile_core.so
  kotlin/
    ... generated com.fresnica.core sources ...
```

The script fails if any expected ABI library is absent. It also fails if `cargo-ndk` copies an unexpected additional `.so` into an ABI directory; dependencies must not silently expand the native package.

A Gradle integration will consume:

- `jniLibs` as an Android `jniLibs` source directory;
- generated Kotlin as a source directory;
- JNA `5.12.0` or newer, required by the stable UniFFI Kotlin backend.

The generated Kotlin layer is not the React Native bridge. A later `FresnicaCoreModule` will translate the small RN-facing API into calls on the generated `MobileCoreApi`.

## Apple

The first Apple package supports:

- `aarch64-apple-ios` for iPhone/iPad devices;
- `aarch64-apple-ios-sim` for Apple Silicon simulators;
- `x86_64-apple-ios` for Intel simulators.

The default minimum iOS deployment target is `13.4`, matching the current Xaman-derived host baseline.

Run on macOS:

```sh
bash bindings/mobile/scripts/build-apple.sh
```

Default output:

```text
bindings/mobile/build/apple/
  device/libfresnica_mobile_core.a
  simulator/libfresnica_mobile_core.a
  generated-swift/
    FresnicaCore.swift
    FresnicaCoreFFI.h
    FresnicaCoreFFI.modulemap
  headers/
    FresnicaCoreFFI.h
    module.modulemap
  FresnicaCoreFFI.xcframework/
```

`FresnicaCoreFFI.xcframework` packages the Rust static FFI library plus the generated C header/module map for device and simulator slices.

`FresnicaCore.swift` remains source code that the host Swift target compiles. It is deliberately not hidden inside the Rust XCFramework. The later iOS native module will import/use this generated Swift API and expose only the intended React Native surface.

## CI

`.github/workflows/mobile-platform-packaging.yml` performs both builds on native GitHub runners:

- Ubuntu installs the exact Xaman-compatible NDK and `cargo-ndk 4.1.2`, then builds all four Android ABIs and Kotlin source.
- macOS builds device/simulator static libraries, generates Swift bindings, creates the FFI XCFramework, and validates the expected output layout.

Both jobs upload their package directories as GitHub Actions artifacts. These artifacts are integration inputs, not release binaries; release signing/versioning remains a later mobile-product concern.

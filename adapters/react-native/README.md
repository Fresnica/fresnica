# Fresnica React Native adapter

This directory is the canonical React Native framework adapter over the generalized Fresnica Native SDK.

It is deliberately separate from `bindings/native`:

```text
fresnica-core
  -> fresnica-sdk
  -> fresnica-native-sdk
  -> Android AAR / Apple Native SDK package
  -> this React Native adapter
  -> application JavaScript
```

The JavaScript module name remains `FresnicaCore` for compatibility with the existing mobile integration contract. The adapter contains framework conversion/lifecycle glue only; it does not own cryptography, signer verification, protected-envelope parsing, Keychain/Keystore storage policy, or WalletUnlockKey policy.

## Exposed framework operations

The canonical adapter exposes the established high-level surface:

- `parseAccount`
- `protectSecret`
- `protectMnemonic`
- `generateMnemonic`
- `reprotect`
- `reveal`
- `prepareEd25519Signing`
- `applyEd25519Signature`
- `canEnrollSystemAuth`
- `hasSystemAuth`
- `removeSystemAuth`
- `enrollSystemAuth`
- `signWithSystemAuth`
- `signWithPasscode`

It intentionally does **not** expose `deriveUnlockKey`, `validateUnlockKey`, or raw routine `signTransactionXdr` to JavaScript.

## One-time Android adapter build

Prerequisites:

- a consumer React Native project with an exact `react-native` version in `package.json`;
- the matching Fresnica Native SDK Android AAR;
- the consumer project's Android Gradle wrapper/toolchain.

Build once in the consumer environment:

```sh
node adapters/react-native/tooling/fresnica-adapter.mjs \
  build react-native \
  --platform android \
  --project /path/to/mobile \
  --native-android-aar /path/to/fresnica-native-sdk.aar \
  --out /path/to/mobile/vendor/fresnica/adapter/react-native
```

The output is:

```text
fresnica-rn-adapter.aar
adapter-manifest.json
```

The adapter AAR is compiled against, but does not embed, React Native or the Fresnica Native SDK. The host continues to link its pinned Native SDK and framework dependencies.

The current Android adapter build expects the host to provide the same external runtime dependencies used by the Native SDK / adapter boundary, including JNA, AndroidX annotation, AndroidX biometric and AndroidX core.

## Compatibility manifest

Generate or validate the manifest independently:

```sh
node adapters/react-native/tooling/adapter-manifest.mjs manifest \
  --project /path/to/mobile \
  --android-aar /path/to/fresnica-rn-adapter.aar \
  --out /path/to/adapter-manifest.json

node adapters/react-native/tooling/adapter-manifest.mjs check \
  --project /path/to/mobile \
  --manifest /path/to/adapter-manifest.json
```

A mismatch reports `adapter rebuild required` rather than silently recompiling the adapter. React Native must be pinned to an exact version for a reproducible adapter binary.

## One-time Apple adapter build

The Apple adapter compiles only the canonical React Native glue against the compiled Native SDK. SDK-owned generated Swift and Keychain/LocalAuthentication sources remain inside `FresnicaSDK`; they are not copied into the adapter.

Prerequisites:

- macOS/Xcode;
- a consumer React Native project with an exact `react-native` version;
- `pod install` completed; the build consumes the React headers/frameworks that CocoaPods actually installed for that React Native version, including the prebuilt `React.xcframework` layout used by current React Native releases;
- matching `FresnicaSDK.xcframework` and `FresnicaSDKFFI.xcframework` Native SDK artifacts.

Build once in the consumer environment:

```sh
node adapters/react-native/tooling/fresnica-adapter.mjs \
  build react-native \
  --platform apple \
  --project /path/to/mobile \
  --native-apple-sdk-xcframework /path/to/FresnicaSDK.xcframework \
  --native-apple-ffi-xcframework /path/to/FresnicaSDKFFI.xcframework \
  --out /path/to/mobile/vendor/fresnica/adapter/react-native
```

The output is:

```text
FresnicaRNAdapter.xcframework
adapter-manifest.json
```

For the real-consumer validation gate, use the one-command wrapper after `pod install`:

```sh
bash adapters/react-native/apple/validate-consumer.sh /path/to/mobile
```

It reuses `bindings/native/build/apple` when present (or runs the Apple Native SDK validator first), compiles the adapter against the consumer's actual CocoaPods integration artifacts, verifies the compatibility manifest, and checks arm64 device plus arm64/x86_64 simulator slices. Source-built React Native is resolved through CocoaPods header trees; prebuilt React Native is resolved through the installed `React.xcframework` plus CocoaPods public/private headers. Fresnica does not reimplement React Native podspec header mapping.

`FresnicaRNAdapter.xcframework` is a static XCFramework containing the Swift adapter implementation and Objective-C React Native registration shim. The normal application build links the pinned adapter binary plus the pinned Fresnica Native SDK; it does not compile adapter source. Keep `-ObjC` in the Apple host linker flags so the React Native registration category/constructor is retained.

The underlying `FresnicaSDK.xcframework` iOS path has passed real macOS/Xcode validation. The complete adapter path has also passed `validate-consumer.sh` against a freshly generated React Native 0.87 project after real CocoaPods installation on macOS, producing arm64 device plus arm64/x86_64 simulator slices. The adapter's iOS-device slice selection explicitly excludes the macOS Native SDK slice now carried by the shared Apple XCFrameworks.

## Legacy Mobile code

`bindings/mobile/platform/**` is frozen v0.1.0 compatibility/integration donor material. New framework work belongs here and targets the released `fresnica-native-sdk`; do not add new product behavior to `fresnica-mobile-core`.

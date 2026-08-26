# Web / WASM SDK Security Boundary

Updated: 2026-08-25

## Purpose

The Web SDK preserves Fresnica Core wallet/signing semantics without pretending that a browser has the same security primitives as Android Keystore or Apple Keychain.

The first browser baseline is therefore **passcode-authorized software signing**. Persistent browser system-auth/passkey authorization is intentionally out of scope until a separate WebAuthn/passkey design is reviewed.

## Ownership

```text
browser application
  - UI / session policy
  - persistence of opaque protected envelopes
  - transaction acquisition / submission
  - explicit collection of passcode or recovery material

fresnica-wasm-sdk
  - mechanical JS/WASM DTO and error mapping
  - browser-safe API filtering

fresnica-sdk / Rust Core
  - identity parsing
  - signer protection
  - KDF / encryption
  - signer identity verification
  - transaction signing
  - Reveal / Export
  - external Ed25519 signature verification
```

The browser application must not parse, edit, or recreate protected signer envelopes.

## Routine signing

`WalletUnlockKey` is a native-process capability and is not a JavaScript API.

Forbidden Web exports:

```text
deriveUnlockKey
validateUnlockKey
signTransactionXdr(envelope, unlockKey, ...)
```

The Web routine path is one composite operation:

```text
signTransactionXdrWithPasscode(
    envelope,
    passcode,
    expectedSignerPublicKey,
    transactionXdr,
    networkPassphrase
)
```

Inside Rust:

1. parse the opaque protected envelope;
2. derive and verify a `WalletUnlockKey` from the fresh passcode;
3. sign with the verified signer;
4. drop/zeroize the unlock key before returning;
5. return only signed transaction XDR.

JavaScript never receives the unlock key.

## Setup and recovery plaintext

`protectSecret`, `protectMnemonic`, and `generateMnemonic` are explicit wallet provisioning operations. `reveal` is an explicit recovery/export operation.

These flows may necessarily place secret/mnemonic/passcode strings in JavaScript memory. Rust can zeroize its own copies but cannot guarantee erasure of immutable JavaScript strings. Applications therefore must:

- keep these values scoped to the explicit operation;
- never write plaintext signing material or passcodes to IndexedDB/localStorage;
- never log or serialize them for diagnostics;
- clear application references as soon as practical;
- require a fresh app passcode for each Reveal / Export call.

## Persistence

A browser wallet may persist:

- account identity;
- signer public key;
- the complete opaque protected signer envelope;
- account/signer references and non-secret application metadata.

It must not persist:

- application passcode;
- `WalletUnlockKey` or equivalent derived KDF output;
- plaintext Stellar secret;
- plaintext mnemonic or mnemonic passphrase.

IndexedDB is a persistence mechanism, not a hardware-backed secret store. The encrypted envelope remains protected by the application passcode and Core's protection format.

## Randomness

The final Web/WASM crate enables `getrandom`'s `wasm_js` backend for `wasm32-unknown-unknown`. In supported browser environments, random bytes are sourced through Web Crypto `Crypto.getRandomValues` via `wasm-bindgen`.

This backend is enabled only at the final browser package. `fresnica-core` and `fresnica-sdk` remain platform-neutral and do not globally opt into a JavaScript randomness backend.

## Validation checkpoint

The browser SDK boundary has been validated end-to-end on macOS with `bindings/wasm/scripts/validate-local.sh`. The validation covers the Rust security-boundary check, `wasm32-unknown-unknown` compilation, release Web package generation, generated JS/TypeScript surface checks, and Node-hosted runtime conformance using the shared transaction vectors.

## External signers

`prepareEd25519Signing` and `applyEd25519Signature` remain public. They expose only transaction hash/context and verify the returned Ed25519 signature before mutating XDR. Transport to a hardware/passkey/external signer remains an application or adapter concern.

## Future passkey / WebAuthn work

Do not store a raw `WalletUnlockKey` as a browser convenience token and do not equate a WebAuthn assertion with native biometric key release.

A future design may evaluate WebAuthn PRF/passkey-derived wrapping or another browser-native mechanism, but it must define:

- key derivation/wrapping semantics;
- origin/RP-ID binding;
- recovery and device migration;
- credential replacement;
- replay/session behavior;
- what Core verifies versus what the browser merely asserts.

Until that design exists, the supported Web software-signer path remains fresh-passcode signing.

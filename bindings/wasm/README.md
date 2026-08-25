# Fresnica Web / WASM SDK

`fresnica-wasm-sdk` is the browser-facing binding over the platform-neutral `fresnica-sdk` contract.

It is a final WebAssembly package, so this crate intentionally enables `getrandom`'s `wasm_js` backend. Randomness is provided by the JavaScript environment's Web Crypto implementation through `wasm-bindgen`; the generic Rust Core and SDK crates do not opt into this browser backend.

## Security boundary

The Web API intentionally does **not** export:

- `derive_unlock_key`
- `validate_unlock_key`
- raw unlock-key-based `sign_transaction_xdr`

Routine protected-software signing uses `signTransactionXdrWithPasscode(...)`. The passcode enters Rust, Core derives and verifies the `WalletUnlockKey`, signs, and drops the key without returning it to JavaScript.

`reveal(...)` remains an explicit exceptional recovery/export operation and requires a fresh application passcode. Secret or mnemonic plaintext returned by `reveal` is intentionally declassified to JavaScript and must not be cached by the SDK.

Wallet creation/import necessarily accepts secret or mnemonic plaintext from JavaScript. JavaScript strings cannot be reliably zeroized by Rust; applications should minimize their lifetime and must not persist them after protection succeeds.

The encrypted signer envelope is opaque application data and may be persisted by a browser client. The browser client must not parse or mutate its cryptographic fields.

No browser biometric/passkey persistence policy is implied by this first WASM package. A WebAuthn/passkey authorization design requires a separate security review.

See `../../docs/wasm-sdk-security.md` for the full boundary.

## Public surface

The first WASM binding exposes:

- `version`
- `parseAccount`
- `protectSecret`
- `protectMnemonic`
- `generateMnemonic`
- `reprotect`
- `signTransactionXdrWithPasscode`
- `reveal`
- `prepareEd25519Signing`
- `applyEd25519Signature`

Byte-oriented transaction/signature arguments and return values use `Uint8Array`. Structured results are native JavaScript objects with camel-case field names. Errors are JavaScript `Error` objects with `name = "FresnicaSdkError"` and the stable Fresnica category in `error.code`.

## Build

Prerequisites:

- current Rust toolchain
- `wasm32-unknown-unknown` target
- `wasm-bindgen-cli` exactly `0.2.127`

Example:

```sh
cargo install wasm-bindgen-cli --version 0.2.127 --locked
bash bindings/wasm/scripts/validate-local.sh
```

`validate-local.sh` checks the source-level security boundary, Rust formatting/tests, the `wasm32-unknown-unknown` target, the generated package/TypeScript export surface, and a Node-hosted runtime conformance pass against the shared transaction-signing vector. The build-only entry point remains `bindings/wasm/scripts/build-web.sh`.

The default output is `bindings/wasm/build/web` and contains ES-module JavaScript, TypeScript declarations, WebAssembly, and package metadata.

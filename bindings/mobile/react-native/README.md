# React Native Core module

`FresnicaCore` is the high-level React Native surface over the mobile Core facade and the native
software-signer authorization services. It deliberately does **not** mirror every UniFFI method.
In particular, React Native never receives `WalletUnlockKey` bytes, biometric `Cipher` objects,
one-shot signing sessions, or the raw unlock-key signing methods.

## JavaScript surface

Android and Apple expose the same promise-based methods.

### Account and protected signer lifecycle

```text
parseAccount(address)
  -> { kind, address, publicKey }

protectSecret(secret, appPasscode, expectedSignerPublicKey?)
  -> { signerPublicKey, envelopeJson }

protectMnemonic(
  mnemonic,
  mnemonicPassphrase,
  index,
  language?,
  appPasscode,
  expectedSignerPublicKey?,
)
  -> { signerPublicKey, envelopeJson }

generateMnemonic(language, strength, mnemonicPassphrase, index, appPasscode)
  -> {
       signer: { signerPublicKey, envelopeJson },
       mnemonic,
       language,
       index,
     }

reprotect(envelopeJson, currentPasscode, newPasscode, expectedSignerPublicKey)
  -> { signerPublicKey, envelopeJson }

reveal(envelopeJson, freshAppPasscode, expectedSignerPublicKey)
  -> {
       kind: "secret" | "mnemonic",
       secret?,
       mnemonic?,
       mnemonicPassphrase?,
       index?,
       language?,
     }
```

`expectedSignerPublicKey` is the identity check used when attaching signing material to an existing
account/signer record. A mismatch is a Core `identity-mismatch`, not a client-side string comparison.

`reprotect` changes the protected envelope and therefore invalidates any previously derived
`WalletUnlockKey`. The caller must first persist the new envelope atomically, then remove/re-enroll
system authentication. This method intentionally does not delete the Keychain/Keystore record before
the client has committed the new envelope.

### External Ed25519 signing

```text
prepareEd25519Signing(transactionXdrBase64, networkPassphrase)
  -> { transactionHashBase64, transactionXdrBase64, networkPassphrase }

applyEd25519Signature(
  transactionXdrBase64,
  networkPassphrase,
  signerPublicKey,
  signatureBase64,
)
  -> signedXdrBase64
```

The external provider receives the public signing request and returns a 64-byte Ed25519 signature.
Core recomputes the transaction hash, verifies the signature against `signerPublicKey`, and only then
appends it to the envelope. No private key or `WalletUnlockKey` is involved.

### Protected software signing

```text
canEnrollSystemAuth() -> boolean
hasSystemAuth(expectedSignerPublicKey) -> boolean
removeSystemAuth(expectedSignerPublicKey) -> true

enrollSystemAuth(envelopeJson, appPasscode, expectedSignerPublicKey) -> true

signWithSystemAuth(
  envelopeJson,
  expectedSignerPublicKey,
  transactionXdrBase64,
  networkPassphrase,
  reason,
) -> signedXdrBase64

signWithPasscode(
  envelopeJson,
  appPasscode,
  expectedSignerPublicKey,
  transactionXdrBase64,
  networkPassphrase,
) -> signedXdrBase64
```

Routine software signing remains native-only. `deriveUnlockKey`, `validateUnlockKey`, and raw
`signTransactionXdr` exist in UniFFI for native platform code but are intentionally absent from the
React Native module.

## Plaintext boundary

Signing material may cross React Native only in the already-defined exceptional lifecycle cases:

- initial secret or mnemonic import;
- one-time mnemonic generation/backup display;
- explicit Reveal / Export after a fresh app passcode.

It must not be persisted as React Native state, logged, sent to analytics, or reused for routine
signing. Native code cannot reliably zero immutable JavaScript/Swift/Kotlin strings after they have
crossed the bridge, which is why this boundary stays exceptional.

Transaction XDR and signatures use base64 at the React Native boundary. Native adapters reject empty
or malformed base64 before calling Core; Ed25519 signatures must decode to exactly 64 bytes. Numeric
mnemonic `index` and `strength` inputs must be finite unsigned 32-bit integers. Core remains
authoritative for mnemonic language/strength semantics, Stellar identities, protected data and XDR.

## Error categories

Stable Core errors are preserved:

- `invalid-input`
- `invalid-passcode`
- `invalid-unlock-key`
- `invalid-protected-data`
- `identity-mismatch`
- `invalid-transaction`
- `core-error`

Platform authorization adds:

- `auth-in-progress`
- `user-cancel`
- `system-auth-unavailable`
- `system-auth-not-enrolled`
- `system-auth-invalidated`
- `system-auth-failed`
- `system-auth-error`
- `native-error`

## Android host integration

The AAR contains `com.fresnica.core.reactnative.FresnicaCoreModule` and
`FresnicaCorePackage`. A React Native host registers the package alongside its other local
`ReactPackage` implementations.

The module owns `BiometricPrompt` and authenticates the exact enrollment/decrypt `Cipher` created by
`WalletUnlockKeyStore`. Prompt cancellation explicitly destroys the pending native session.

## Apple host integration

`FresnicaCoreModule.swift` owns the high-level Core/native service. `FresnicaCoreModule.m` contains
only `RCT_EXTERN_MODULE` declarations. Add both files, `FresnicaSignerAuthorization.swift`,
`FresnicaWalletUnlockKeyStore.swift`, generated `FresnicaCore.swift`, and
`FresnicaCoreFFI.xcframework` to the application target.

Keychain access performs the actual Face ID / Touch ID operation that releases a per-signer
`WalletUnlockKey`; a separate biometric-success flag is never treated as signing authorization.

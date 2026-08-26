# Application Security Capability

Maturity: **Defined**

## Purpose

Application Security is the reusable application/platform boundary that supports security-related Flows without moving cryptographic authority out of Fresnica Core/SDK.

Examples include Security Settings, routine-signing authorization, passcode change and secure cleanup/recovery workflows.

## Agreed boundary

Implementations may expose platform-appropriate capabilities for:

- passcode verification/re-protection coordination;
- application lock/session authorization state;
- system-auth availability and enrollment/removal;
- secure storage of platform authorization artifacts;
- post-reprotection cleanup/registration work;
- recovery and security-settings outcomes.

## Non-ownership

Application Security does not own:

- KDF parameters or encryption algorithms;
- secret/mnemonic derivation;
- protected-envelope cryptographic meaning;
- transaction hashing/signing;
- Reveal/Export credential policy;
- platform UI wording or biometric prompt design.

Those boundaries are defined by Fresnica SDK/Core and the product Flow contracts.

## Reference Semantics: Native SDK Apple/Android system auth

Fresnica already has a reviewed Native SDK system-auth design with Apple and Android implementations:

- [Mobile System Authentication / WalletUnlockKey Contract](../platforms/mobile/system-auth.md)
- [`bindings/native/platform/android/src/main/java/com/fresnica/sdk/security/WalletUnlockKeyStore.java`](../../bindings/native/platform/android/src/main/java/com/fresnica/sdk/security/WalletUnlockKeyStore.java)
- [`bindings/native/platform/android/src/main/kotlin/com/fresnica/sdk/security/FresnicaSignerAuthorization.kt`](../../bindings/native/platform/android/src/main/kotlin/com/fresnica/sdk/security/FresnicaSignerAuthorization.kt)
- [`bindings/native/platform/apple/FresnicaWalletUnlockKeyStore.swift`](../../bindings/native/platform/apple/FresnicaWalletUnlockKeyStore.swift)
- [`bindings/native/platform/apple/FresnicaSignerAuthorization.swift`](../../bindings/native/platform/apple/FresnicaSignerAuthorization.swift)

These are strong reference semantics for native products, but the Application Security capability remains Defined until application-level experience across products proves which parts should be frozen cross-platform.

### 1. Fresnica passcode has higher privilege than system auth

System authentication is a device-local authorization convenience for routine signing. It is not the Fresnica passcode, not a recovery credential and not sufficient by itself for Reveal/Export or passcode rotation.

### 2. System auth gates signer use, not raw secret access

The Native SDK model keeps `WalletUnlockKey` and OS cryptographic objects in native memory. React Native/application code requests a high-level reviewed signing action and receives the signed result, not the unlock key, mnemonic, secret or biometric cipher/key object.

### 3. Registration is signer-scoped while the protection domain may be device-scoped

A device may have one OS-backed protection domain while each protected software signer keeps an independent wrapped authorization record. Sharing the OS protection domain does not merge signer identities or Core envelopes.

### 4. System-auth invalidation must fail closed and preserve recovery through the Fresnica passcode

Biometric enrollment changes, deleted OS keys, cancellation or unavailable system auth must not silently downgrade security or expose signing material. The product may fall back to the Fresnica passcode path when policy allows it.

### 5. Passcode rotation invalidates stale system-auth registrations

Re-protection changes the protected signer envelope/unlock-key relationship. Old system-auth registrations must not remain presented as usable for the new envelope state. Re-registration may be retried after the atomic durable passcode-rotation commit.

### 6. One product passcode over multiple software signers changes as one security state

If a Fresnica product presents one app passcode as protecting a set of software signers, passcode rotation must not silently leave a mixed state where some signers require the old passcode and others require the new one.

The durable semantic transition must either be atomic for the protected signer set or enter an explicit recoverable migration state that prevents ambiguous routine signing until the migration is completed/recovered. A storage engine need not literally provide one database transaction, but partial success must be visible and safe rather than masquerading as completed rotation.

## Implementation-specific choices today

The following are platform mechanisms, not common contract requirements:

- Android RSA/OAEP vs Apple P-256/ECIES wrapping;
- AndroidKeyStore, Keychain, `BiometricPrompt`, `LAContext` and framework APIs;
- exact OS key aliases and storage records;
- exact biometric policy flags;
- prompt copy and screen flow;
- one device protection domain as the only possible future desktop/mobile implementation strategy.

## Candidate semantics for promotion

1. System auth is lower-privilege, device-local routine-signing authorization rather than a recovery credential.
2. Raw unlock/secret material stays below the application/JS boundary.
3. Authorization artifacts are signer-scoped even when protected by shared device infrastructure.
4. Invalidation/cancellation fails closed and preserves explicit fallback semantics.
5. Passcode/re-protection changes invalidate stale authorization registrations safely.
6. A product-wide passcode rotation over multiple software signers is atomic in wallet meaning or enters an explicit recoverable migration state; silent mixed old/new passcode state is invalid.

## Relationship to Flows

The Security Settings Flow owns user-facing enable/disable/change/recovery policy and confirmation. Transaction Flows consume Signing Coordination rather than calling biometric/passcode mechanisms independently.

## Promotion criteria

Keep this capability Defined until concrete Mobile/Desktop products demonstrate a stable common application-level contract across materially different Keychain/Keystore/system-auth environments. Native SDK mechanics alone should not force product-level API shape.

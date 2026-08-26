# Signer Capability

Maturity: **Normative**

## Purpose

The Signer Capability defines a signing capability that may be associated with one or more account identities.

It does not equate signer identity with account identity and does not standardize one storage implementation.

## Signer kinds

The shared model must be able to represent at least:

- protected software signer;
- hardware/external signer;
- provider/contract-specific signer capability where explicitly supported.

A protected software signer has a public signer identity plus an opaque protected envelope. Raw private material is not durable application state.

## Required semantics

A conforming implementation must preserve:

1. signer identity is explicit and independently verifiable;
2. protected software signer material is created/validated by Fresnica SDK/Core;
3. watch-only means no applicable local signer capability;
4. attaching recovery material to an existing expected signer identity must fail with identity mismatch before durable state changes when the derived key does not match;
5. one account may eventually reference multiple signers and one signer may be referenced by multiple accounts; application data models must not make one-to-one identity equality a permanent invariant;
6. external/hardware signers do not receive a software-signer envelope or `WalletUnlockKey`;
7. signer provider failures must not be confused with cryptographic acceptance of a returned signature.

## Software signer lifecycle

Conceptually:

```text
secret/mnemonic input
   -> SDK/Core validate + derive signer identity
   -> SDK/Core protect
   -> opaque protected signer result
   -> application persists opaque envelope + public metadata
```

Routine signing uses the protected signer through the Signing Coordination and SDK/Core path. Reveal/Export is a separate higher-privilege Flow.

## External signer lifecycle

External providers use the provider-neutral prepare/apply boundary where possible:

```text
SDK/Core prepare signing request
   -> provider obtains user authorization/signature
   -> SDK/Core verifies/applies returned signature
```

Transport discovery, HID/BLE/browser APIs and provider UI are implementation-specific.

## Errors

Stable semantic categories should distinguish:

- invalid signer/recovery input;
- invalid protected data;
- invalid passcode/unlock authorization;
- identity mismatch;
- unsupported signer mode;
- provider failure;
- invalid returned signature/transaction.

## Security boundary

Secrets, mnemonics, app passcodes and native unlock keys must not be placed in Feature/Flow state, navigation parameters, logs or analytics.

See also:

- [Account Capability](account.md)
- [Signing Coordination](signing-coordination.md)
- [External Signer](external-signer.md)
- [Core Security Boundary](../core-security-boundary.md)

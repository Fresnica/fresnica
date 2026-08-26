# Signing Coordination Capability

Maturity: **Normative**

## Purpose

Signing Coordination defines the application-level sequence that connects a confirmed transaction intent to an authorized signer and then to Fresnica SDK/Core signing.

It prevents every Flow from inventing its own passcode/biometric/hardware policy.

## Ownership split

```text
Flow
  decides THAT signing is required and owns confirmation UX

Signing Coordination
  resolves WHICH signer capability may be used now
  coordinates application/platform authorization

Platform authorization
  obtains biometric/passcode/hardware/provider authorization according to product policy

Fresnica SDK/Core
  defines cryptographic signing meaning and verifies results
```

## Required semantics

A conforming implementation must:

1. resolve authorization/signing state at the time of signing rather than trusting stale UI state;
2. reject watch-only/no-applicable-signer writes;
3. preserve the exact prepared transaction/review binding;
4. keep raw software secrets and native unlock keys out of ordinary Flow state;
5. treat system authentication as an authorization mechanism, not a replacement cryptographic secret format;
6. use SDK/Core for protected-software signing;
7. verify external signatures through the SDK/Core prepare/apply boundary where applicable;
8. preserve the stronger fresh-passcode boundary for Reveal/Export rather than treating a previously authorized routine-signing session as sufficient;
9. consume Ledger Authorization requirements when a transaction/account needs multiple/typed authorization conditions, and not declare signing complete while required weight/conditions remain unsatisfied.

## Software signer authorization

A product may support passcode signing, system-auth-assisted signing or both. The user-facing choice is platform policy, but successful authorization must produce a signing operation that remains scoped to the intended signer and current protected envelope/version.

Stale system-auth registrations/unlock keys must fail safely after re-protection or passcode rotation.

## External signer authorization

Hardware/provider confirmation belongs to the provider/platform implementation. Signing Coordination still binds the provider result to the expected signer and transaction through SDK/Core verification.

## Errors

Flows should receive stable distinctions for:

- no applicable signer/watch-only;
- authorization cancelled/denied;
- invalid passcode/unlock key;
- stale/invalid protected data;
- signer identity mismatch;
- provider failure;
- invalid transaction/signature;
- unsupported signing mode.

## Related contracts

- [Signer](signer.md)
- [Ledger Authorization](ledger-authorization.md)
- [Transaction](transaction.md)
- [Application Security](application-security.md)
- [Core Security Boundary](../core-security-boundary.md)

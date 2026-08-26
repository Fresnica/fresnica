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

## System Auth

System authentication is a platform authorization mechanism. It must not be described as the Fresnica wallet encryption password or as a portable replacement for the Fresnica passcode.

A platform may use system auth to gate access to a signer-scoped native authorization artifact for routine signing. Re-protection/passcode rotation must invalidate stale registrations safely.

## Relationship to Flows

The Security Settings Flow owns user-facing enable/disable/change/recovery policy and confirmation. Transaction Flows consume Signing Coordination rather than calling biometric/passcode mechanisms independently.

## Promotion criteria

Keep this capability Defined until multiple native products demonstrate a stable common application-level contract across materially different Keychain/Keystore/system-auth environments.

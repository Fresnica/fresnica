# Transaction Capability

Maturity: **Normative**

## Purpose

The Transaction Capability defines the shared lifecycle for preparing, reviewing, signing and submitting Stellar transactions.

It is the common integrity boundary below transaction-producing Application Flows.

## Lifecycle

Conceptually:

```text
intent
  -> prepare exact transaction
  -> immutable semantic review
  -> product confirmation
  -> resolve current authorization/signing capability
  -> sign exact prepared transaction
  -> submit
  -> normalized result
```

The Flow may cancel before authorization. It must not silently mutate transaction meaning after review.

## Transaction integrity

A conforming implementation must preserve:

1. review corresponds to the exact transaction/envelope that will be signed;
2. network identity/passphrase is part of transaction signing context;
3. source sequence and current ledger parameters are obtained close enough to preparation to avoid knowingly stale writes;
4. fees reflect the actual operation count/current fee policy used for the prepared transaction;
5. signing goes through Fresnica SDK/Core for Core-owned cryptographic operations;
6. external signatures are verified against the expected signer and exact transaction context before being accepted.

## Amounts and XDR

Transaction-building implementations may use different Stellar SDKs. Cross-platform code does not need to share Rust XDR types.

Where XDR crosses an SDK/provider boundary, it is an exact opaque transaction representation and must not be reinterpreted through lossy conversions merely to satisfy a provider API.

## Submission result

A normalized submission result must distinguish at least:

- confirmed/accepted submission with transaction identity and ledger when known;
- deterministic rejection;
- uncertain submission where transport failure leaves chain acceptance unknown.

An uncertain submission must not be treated as a normal retryable failure. A product should protect against accidental duplicate writes until the transaction hash is reconciled or the uncertainty policy expires.

The current Rust reference persists only public pending-transaction metadata for this guard.

## Stable security errors

Core/SDK-owned errors include stable categories such as:

- `invalid-transaction`;
- `invalid-passcode`;
- `invalid-unlock-key`;
- `invalid-protected-data`;
- `identity-mismatch`;
- `core-error`.

Network/submission errors should be normalized separately from these cryptographic categories.

## Conformance

Classic transaction cryptographic compatibility is fixed by [`../../spec/test-vectors/transaction-signing-v1.json`](../../spec/test-vectors/transaction-signing-v1.json).

Application-level fixtures should additionally test review/signing binding and uncertain-submission behavior where implementations expose it.

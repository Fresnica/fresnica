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

An uncertain submission must not be treated as a normal retryable failure. A product must protect against accidental duplicate writes until the transaction identity/hash is reconciled or an explicit uncertainty policy permits retry.

Where the implementation can identify the exact submitted transaction, recovery should prefer querying/reconciling that transaction's chain status before constructing or submitting a replacement. Transport timeout alone is not evidence that the chain rejected the transaction.

The duplicate guard/reconciliation state should contain only public transaction metadata needed for recovery; it must not persist secrets, passcodes, unlock keys or other signing material. The current Rust/RefPython references provide implementation evidence for pending-submission recovery without making one storage schema normative.

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

# Protected Software Transaction Signing

Maturity: **Normative**

Contract ID: `fresnica-protected-software-transaction-signing-v1`

This is the Mobile contract for routine signing with a local protected software signer. Native SDK v0.3.1 and the canonical React Native adapter conform to it.

## Entry point

Mobile uses the high-level adapter method:

```text
signWithPasscode(
  envelopeJson,
  appPasscode,
  expectedSignerPublicKey,
  transactionXdrBase64,
  networkPassphrase
) -> signedTransactionXdrBase64
```

`signWithSystemAuth` has the same transaction/network/signer binding after platform authorization succeeds.

The transaction signer is operation-agnostic. The same method accepts reviewed Classic and Soroban `TransactionEnvelope` XDR, including `InvokeHostFunction` with Soroban transaction data.

Mobile must not create a separate Soroban final-transaction signer.

## Required bindings

The adapter must pass the exact values below to Native SDK/Core:

- protected signer envelope;
- expected signer public key;
- reviewed transaction-envelope bytes;
- selected network passphrase.

Core must reject a protected signer whose decrypted public key does not equal `expectedSignerPublicKey`.

The signer does not have to equal the transaction source account. Ledger Authorization determines whether that signer is valid for the account/threshold being authorized.

The network passphrase is part of the Stellar transaction signature base. Mobile must supply the selected network explicitly; it must not infer or substitute it.

Signing may append a `DecoratedSignature`. It must not change the transaction body, including source account, sequence, fee, preconditions, memo, operations, Soroban authorization entries, Soroban resources/footprint, or resource fee.

The transaction hash under the same network must be identical before and after signing.

## Soroban authorization entries

Final transaction-envelope signing and detached `SorobanAuthorizationEntry` signing are separate domains.

If an invocation needs detached G-address authorization, Mobile completes that authorization through the Soroban authorization signing API first. It then signs the final reviewed transaction envelope through `signWithPasscode` / `signWithSystemAuth`.

## Error behavior

Mobile must preserve these failures rather than retrying with another signer or network:

- invalid passcode;
- invalid/stale protected signer data;
- signer identity mismatch;
- invalid transaction XDR;
- unsupported authorization mode.

A valid transaction must not become `unsupported` only because it contains Soroban operations.

## Conformance fixture

[`protected-soroban-signing-v1.json`](../../../spec/test-vectors/protected-soroban-signing-v1.json) is a captured Blend Testnet V2 `get_config` invocation.

It fixes:

- source account and expected signer;
- Blend contract ID and function;
- exact simulated Soroban transaction XDR;
- Testnet transaction hash;
- the different hash produced by the Mainnet passphrase;
- exact expected signed envelope.

The fixture is stale for submission. It is used only for signing conformance.

Android and iOS release-package tests must pass this same vector before this contract is considered supported by a published Native SDK version.

# React Native account / signer lifecycle

`src/wallet-lifecycle.ts` is the first client-side implementation of the Mobile/Core account and
signer model. It is intentionally storage-neutral: the future Fresnica app may adapt Realm, SQLite,
or another local database through `WalletStore` without changing Core or the lifecycle semantics.

The persisted concepts are separate:

```text
AccountRecord
  id
  address
  kind
  network
  name

ProtectedSoftwareSignerRecord
  id
  signerPublicKey
  envelopeJson

AccountSignerReference
  accountId -> signerId
```

There is no persisted `watchOnly` or `walletType` flag. `hasLocalSigner(accountId)` is derived from
whether the account has local signer references. This is deliberately not named `canSign`: actual
Stellar signability also depends on current ledger signer weights and thresholds.

## Watch-only flows

Adding watch-only first calls native `parseAccount`, then persists only `AccountRecord`. It requires
no passcode, recovery material, signer envelope, or `WalletUnlockKey`.

Upgrading a classic watch-only account calls native `protectSecret` or `protectMnemonic` with the
existing account address as `expectedSignerPublicKey`. Core therefore owns the identity check. Only
a successful result is persisted as a new signer plus account-signer reference; the existing account
record and its stable id remain unchanged.

`C...` contract accounts are not upgraded by attaching an Ed25519 software signer. Contract/passkey
authorization remains a separate future capability.

Downgrading removes every local signer reference for the account. An unreferenced signer record and
its opaque envelope are deleted, while a signer shared with another account remains. Native system
auth is removed only for signer records that became orphaned.

Database deletion commits before Keychain/Keystore cleanup. This ordering ensures that a cleanup
failure cannot restore or retain the encrypted signer envelope. `pendingSystemAuthCleanup` reports
signer public keys whose secure-store cleanup should be retried by the host application.

## Store contract

A `WalletStore` adapter must provide atomic `transaction` semantics. Reads used to decide whether a
signer became orphaned occur inside the same transaction as reference/signer deletion. This is what
prevents a downgrade of one account from deleting a signer still referenced by another account.

The included tests use an in-memory adapter only. It is not intended as production persistence.

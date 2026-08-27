# React Native account / signer lifecycle

`src/wallet-lifecycle.ts` is the client-side implementation of the Mobile/Core account and signer
model. Core owns identity/crypto semantics; the mobile client owns persistence and lifecycle policy.

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

## Realm persistence

`src/realm-wallet-store.ts` provides the first production-shaped `WalletStore` adapter. It exports
three independent Realm schemas and schema version `1`:

- `FresnicaAccountRecord`
- `FresnicaSignerRecord`
- `FresnicaAccountSignerReference`

The host application owns the actual Realm instance, configuration, file location, encryption,
schema composition and migrations. `RealmWalletStore` only consumes the small database surface it
needs, so the lifecycle layer does not import or pin a Realm package version.

The reference table uses a deterministic composite primary key while keeping `accountId` and
`signerId` indexed. Signer envelopes remain opaque strings; plaintext secret/mnemonic fields do not
exist in these schemas.

Because Fresnica has not had a public wallet release yet, schema version 1 has no legacy migration.
After the first public release, schema changes require an explicit migration and rollback plan.

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

## App-passcode rotation

`reprotectAllProtectedSigners(currentPasscode, newPasscode)` is a staged client transaction:

1. snapshot every protected software signer;
2. call Core `reprotect` for every envelope without changing persistence;
3. if every Core operation succeeds, verify the signer snapshots are still current;
4. replace all envelopes in one `WalletStore.transaction`;
5. only after commit, remove stale system-auth `WalletUnlockKey` records;
6. return the signer keys that need system-auth re-enrollment and any cleanup failures to retry.

A wrong current passcode or any Core failure therefore leaves every persisted envelope on the old
passcode. A concurrent signer modification aborts the database swap. A Keychain/Keystore cleanup
failure never rolls the newly committed envelopes back.

## Portable backup / restore

`src/portable-backup.ts` defines the Mobile reference for `fresnica-wallet-backup` version 2 without
copying Realm primary keys or device-bound System Auth state into the file. Backup references are
local to the backup (`a1`, `s1`, ...), and stored networks are only suggestions.

Restore must receive an explicit target network for every account. Protected signer envelopes are
validated and freshly re-encrypted by Core through `reprotect(oldPasscode, newPasscode,
expectedSignerPublicKey)` before they can be persisted. Direct Classic master-key references can be
activated from identity equality; delegated Classic and contract/provider references remain pending
until the host validates their authorization. Recovery-source grouping is returned as a pending
hint and is not persisted as signer authority by this layer.

This keeps backup crypto in Rust Core and keeps the React Native layer limited to graph validation,
staging and one atomic store transaction.

## Store contract

A `WalletStore` adapter must provide atomic `transaction` semantics. Reads used to decide whether a
signer became orphaned or whether a staged re-protection is still current occur inside the same
transaction as the writes they protect.

Tests include an in-memory lifecycle store plus a Realm-compatible database fake. The latter verifies
the Realm schema/adapter behavior without tying CI to a particular Realm native package build.

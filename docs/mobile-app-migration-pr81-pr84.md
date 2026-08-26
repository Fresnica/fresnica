# Mobile App Migration Guide for PR #81-#84

Status: **handoff source for the independent Fresnica Mobile application**.

This document identifies the React Native/application-side work that was implemented temporarily inside the `fresnica` repository during PR #81-#84. The independent Mobile project should use this document to decide what to absorb, adapt, or leave behind.

Before absorbing any application-side code, Mobile should establish its SDK/framework boundary using `docs/mobile-framework-adapter-contract.md`. That document is the authoritative integration baseline: pin compiled Native SDK binaries, compile the canonical framework adapter once in the Mobile framework environment, store the generated adapter binaries/manifest, and keep adapter-source compilation out of ordinary app builds.

The repository boundary going forward is:

```text
fresnica
  Core + compiled native SDK + canonical framework adapters + conformance tests

fresnica-mobile
  React Native application + generated RN adapter binaries + Realm configuration/migrations
  + account/signer persistence + product lifecycle/orchestration
  + screens/navigation/network/product state
```

The files described below are donor/reference implementation for the Mobile application. They are not a requirement that the independent app preserve the same TypeScript class names or Realm schema syntax.

## Rules that must survive migration

These are architecture/security contracts, not implementation details:

1. **Account != Signer != Recovery Source.** An AccountRecord represents on-chain identity; SignerRecord represents an available signing capability; a Recovery Source is Mobile-owned grouping metadata for shared mnemonic backup/HD UX. None is a synonym for another.
2. **Watch-only is derived state.** A watch-only account is an account with no local signer reference. Do not persist a second `watchOnly`/`walletType` truth that can drift.
3. **Classic account identity and signer identity may differ.** Delegated/additional/multisig signers are valid. Do not globally assume `account.address == signerPublicKey`.
4. **Direct master-key watch-only upgrade is identity-checked by Core.** Pass `expectedSignerPublicKey = account.address` when attaching a secret/mnemonic intended to become that account's master signer. Do not duplicate the cryptographic check in JavaScript.
5. **C... contract accounts are identity-only for the current Ed25519 software-signer path.** Do not attach a classic software signer and pretend this implements contract/passkey authorization.
6. **Protected signer envelopes are opaque.** Mobile persists `envelopeJson`; it does not parse or mutate its cryptographic fields.
7. **Routine signing remains native-only.** `WalletUnlockKey`, biometric Cipher/session objects and raw `signTransactionXdr` do not enter JavaScript.
8. **Reveal/Export requires a fresh app passcode.** System authentication / stored WalletUnlockKey is not an export credential.
9. **Passcode rotation is staged then atomic.** Re-protect every protected signer first, commit all new envelopes in one persistence transaction, then replace wrapped unlock-key records in the existing device System Auth Domain. This post-commit registration does not require another biometric prompt.
10. **Plaintext mnemonic/secret is exceptional and ephemeral.** Initial import, one-time mnemonic generation/backup display and explicit Reveal/Export are the only intended crossings. Never persist them to Realm, Redux/navigation state, logs, analytics or crash reports.

## PR #81 - Account / Signer lifecycle coordinator

Source:

- `bindings/mobile/react-native/src/wallet-lifecycle.ts`
- `bindings/mobile/react-native/test/wallet-lifecycle.test.ts`

### Absorb into the Mobile project

Absorb the **behavior and invariants** of:

- `AccountRecord`
- `ProtectedSoftwareSignerRecord`
- `AccountSignerReference`
- watch-only account creation through native/Core `parseAccount`
- `hasLocalSigner(accountId)` semantics
- classic watch-only upgrade with secret or mnemonic
- downgrade to watch-only without deleting the AccountRecord
- orphan-only signer deletion
- shared/delegated signer preservation
- post-commit system-auth cleanup

The current `WalletStore` interfaces are suitable as a reference for the persistence port. If the Mobile app already has a repository/store abstraction, map these operations into that abstraction rather than creating a duplicate layer.

### Adapt rather than copy blindly

- record ID generation should follow the Mobile application's existing ID strategy;
- account metadata may grow beyond `network` and `name`;
- a future signer model should allow additional signer kinds without forcing every signer into `ProtectedSoftwareSignerRecord`;
- `hasLocalSigner` must not be renamed to `canSign`: actual Stellar authorization depends on current ledger signer weights and thresholds.

### Mandatory lifecycle behavior

Watch-only upgrade must snapshot the account, ask Core to protect/verify the signing material, then atomically attach the signer only if the account is still watch-only and its identity has not changed.

Downgrade removes account-to-signer references first. A signer record is deleted only when no other account references it. Keychain/Keystore cleanup occurs after the database commit; cleanup failure is retryable and must not restore the deleted envelope.

## PR #82 - Realm-ready persistence and global passcode rotation

Source:

- `bindings/mobile/react-native/src/realm-wallet-store.ts`
- updates in `wallet-lifecycle.ts` for staged/atomic `reprotectAllProtectedSigners`
- related tests under `bindings/mobile/react-native/test/`

### Absorb into the Mobile project

Use the current Realm code as a **schema and transaction reference**, especially the separation of:

```text
AccountRecord
SignerRecord
AccountSignerReference
RecoverySourceRecord / grouping metadata (when useful)
```

The Mobile project owns the actual Realm instance, schema version, migration functions, compaction policy, encryption-at-rest choice and app startup/open/close lifecycle.

### Adapt rather than copy blindly

Do not import the current adapter and then create a second Realm configuration around it. Prefer one host-owned Realm configuration in the Mobile project and adapt the store implementation to it.

Schema names/field names may change during absorption, but the following must remain true:

- no mnemonic/private key/WalletUnlockKey column;
- protected software signer stores only public signer identity plus opaque encrypted envelope;
- watch-only state is derived from missing signer references;
- AccountRecord survives signer detach/downgrade;
- the model does not prevent one signer from being referenced by multiple accounts or future multiple signers per account.

### Passcode rotation invariant

The sequence is:

```text
1. Read all protected software signer snapshots.
2. Core.reprotect(oldEnvelope, currentPasscode, newPasscode, signerPublicKey) for every signer.
3. If any Core call fails: write nothing.
4. In one Realm transaction, re-check snapshots and replace every envelope.
5. Commit.
6. Mark all previous signer system-auth registrations stale for the new envelope generation.
7. Derive each new verified WalletUnlockKey from the committed envelope + new passcode.
8. If a System Auth Domain exists, register/replace every signer wrapped key with that domain public key. No biometric prompt is required.
```

A registration failure in step 8 is recorded for retry. It never rolls the database back to old envelopes; that signer remains system-auth unavailable and temporarily falls back to passcode signing. Mobile must not treat an old OS record as current after its envelope changed. If the domain itself is invalidated, initialize one replacement domain once, then register all signers.

### Do not migrate to the Mobile app

The CI path-filter changes from PR #82 belong to the `fresnica` SDK repository. The independent Mobile repository should define its own CI around its own source tree.

## PR #83 - New account import / generation provisioning

Source:

- `bindings/mobile/react-native/src/account-provisioning.ts`
- `bindings/mobile/react-native/test/account-provisioning.test.ts`

### Absorb into the Mobile project

Absorb the coordinator semantics for:

- import Stellar `S...` secret;
- import mnemonic + passphrase + account index + language;
- generate mnemonic through native/Core;
- create AccountRecord + protected SignerRecord + reference in one transaction;
- derive the new account identity from Core/native output rather than reproducing StrKey/derivation logic in JavaScript;
- keep system-auth signer registration as an explicit post-persistence action. `initializeSystemAuth` is device-level and happens once; later `registerSignerSystemAuth` verifies the passcode and does not prompt for biometrics.

### Atomic persistence invariant

A newly provisioned software wallet appears in persistence as one graph:

```text
AccountRecord
    |
AccountSignerReference
    |
SignerRecord(protected-software, signerPublicKey, envelopeJson)
```

Record collision or transaction failure must leave none of the three records partially committed.

### Generated mnemonic handling

The generated mnemonic is returned only for the one-time backup/confirmation flow. The Mobile project must not persist it. The durable artifact is the protected signer envelope.

### HD multi-account behavior

The normal first mnemonic-backed signer uses `index = 0`. If the user adds another address from the same recovery source, do not Reveal or ask for the mnemonic again. Use Native SDK `deriveMnemonicSigner(sourceEnvelope, appPasscode, expectedSourceSignerPublicKey, index)` and persist the returned signer/account graph atomically. Core authenticates the source and derives the requested index internally. Mobile may attach both signers to the same Recovery Source grouping for backup UX.

## PR #84 - Explicit signer Reveal / Export

Source:

- `bindings/mobile/react-native/src/signer-export.ts`
- `bindings/mobile/react-native/test/signer-export.test.ts`

### Absorb into the Mobile project

Absorb this flow as **signer-centric**, not account-centric:

```text
user explicitly chooses Export
  -> resolve local protected-software signer
  -> request fresh Fresnica app passcode
  -> native/Core reveal(envelopeJson, passcode, signerPublicKey)
  -> display/copy/export only inside explicit UX
  -> discard plaintext
```

A missing signer fails before calling Core. Wrong passcode is a Core error. Export never modifies the stored envelope.

### Do not weaken this boundary

Do not add a "Face ID export" shortcut using the stored WalletUnlockKey. System auth authorizes routine signer use; it does not authorize declassification of the recovery secret.

## What remains in `fresnica` and should be consumed, not copied

The independent Mobile application should consume or generate these through the Fresnica SDK integration contract:

- `core/rust` - cryptographic/signing authority behind released native binaries;
- `bindings/native/src` - generalized UniFFI Native SDK binding over `fresnica-sdk`;
- compiled Android Native SDK binary with Rust libraries/native signing implementation;
- compiled Apple Native SDK binary with native signing implementation;
- canonical React Native adapter source and adapter-build recipe/tooling;
- future canonical Flutter/other framework adapter source;
- stable error categories and API version query;
- cross-language transaction/protection vectors and adapter conformance tests.

Do not fork Core/native SDK implementation into the Mobile repository unless there is a deliberate SDK fork. Mobile should pin a released Fresnica Native SDK version.

The canonical RN adapter source remains Fresnica-owned, but Mobile compiles it **once** against its selected RN/toolchain version and stores the generated Android/iOS adapter binaries plus compatibility manifest. Ordinary Mobile builds consume those binaries; they do not rebuild Rust/Core/UniFFI or adapter source.

## Suggested absorption order in `fresnica-mobile`

1. Choose/pin the Mobile React Native version and Fresnica Native SDK/Binding API.
2. Follow `docs/mobile-framework-adapter-contract.md` to compile the RN adapter once, store its binaries/manifest and prove `parseAccount` from React Native.
3. Define the host Realm schema/migration around Account / Signer / Reference plus optional Recovery Source grouping.
4. Establish the Fresnica app passcode; optionally initialize the device System Auth Domain once.
5. Absorb #81 watch-only create, attach and downgrade semantics.
6. Absorb #83 create/import/generate provisioning and v0.2 `deriveMnemonicSigner` HD-add-account behavior. Register new signers into an existing System Auth Domain after persistence/passcode verification, without another biometric prompt.
7. Absorb #82 global passcode rotation into Settings/security UX using all-signer staged `reprotect` + one atomic commit + post-commit wrapped-key replacement.
8. Absorb #84 explicit Reveal/Export UX; system auth alone never authorizes it.
9. Add ledger-side signer/threshold resolution so `hasLocalSigner` can be combined with actual on-chain authorization.

## Completion criteria before deleting donor TypeScript from `fresnica`

The donor `bindings/mobile/react-native/src/*.ts` application orchestration may be removed from `fresnica` after the independent Mobile repository has tests proving:

- the pinned Native SDK + generated adapter binary integration is reproducible from the recorded manifest;
- normal Mobile builds do not compile Rust/Core/UniFFI or adapter source;
- watch-only create/upgrade/downgrade;
- Account != Signer != Recovery Source preservation;
- shared signer detach safety;
- atomic create/import/generate persistence;
- atomic all-signer passcode rotation;
- one-time device System Auth Domain initialization plus no-biometric signer registration and post-rotation wrapped-key replacement;
- fresh-passcode-only Reveal/Export;
- no secret/mnemonic/WalletUnlockKey persisted in application state.

Until then these files remain migration reference, not the place for new Mobile product work.

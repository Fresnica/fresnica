# Backup / Restore Capability

Maturity: **Defined**

## Purpose

Backup / Restore defines the portable wallet-security semantics for moving recoverable Fresnica state between application installations without turning backup into a secret-export shortcut.

It is separate from platform backup APIs and from the Wallet aggregate because future products may model Accounts, Signers and Recovery Sources independently.

## Agreed boundary

A conforming implementation must preserve these invariants:

1. **Account identity, Signer capability and Recovery Source remain distinct.** Backup must preserve their relationships without collapsing them into one wallet record model.
2. **Watch-only stays watch-only.** Backing up or restoring a watch-only account must not invent local signer or recovery material.
3. **Protected signer material stays protected.** Portable backup must not decrypt a protected software signer merely to serialize it into another format.
4. **Restore is not blind deserialization.** Security-significant identity/relationship metadata must be integrity-protected or independently revalidated/reconfirmed before the restored capability is activated.
5. **Signer identity is revalidated before activation when protected signing material is present.** A restored encrypted signer must not become authoritative merely because its outer record contains a plausible `G...` value.
6. **Device-bound authorization is not portable signer authority.** Keychain/Keystore/System Auth wrappers, biometric registrations and device-local wrapped `WalletUnlockKey` artifacts must not be copied into a portable backup as if they were transferable signer capability.
7. **Restore must fail closed on unsupported format/security semantics.** A product may migrate an older format deliberately; it must not silently reinterpret it as a newer security graph.

## Reference Semantics: terminal backup v1

Rust and RefPython deliberately share the current terminal format marker:

```text
format  = fresnica-wallet-backup
version = 1
```

Reference evidence:

- [`clients/rust-client/src/storage.rs`](../../clients/rust-client/src/storage.rs)
- [`clients/rust-cli/src/main.rs`](../../clients/rust-cli/src/main.rs)
- [`reference/python/fresnica/wallet_backup.py`](../../reference/python/fresnica/wallet_backup.py)
- [`reference/python/fresnica/manager.py`](../../reference/python/fresnica/manager.py)
- [`reference/python/tests/test_wallet_backup.py`](../../reference/python/tests/test_wallet_backup.py)

The useful common behavior is that software-signing material remains encrypted and watch-only records contain no signing material. Both references also validate the structural record before saving it.

### Current v1 limitation

The terminal v1 format is **reference evidence, not the future cross-platform portable schema**.

Its protected signer envelope is cryptographically protected, but the surrounding `WalletRecord` metadata is not one authenticated cross-platform envelope. Fields such as network, record format and the account-to-signer relationship therefore cannot be trusted solely because the encrypted signer blob itself was not modified.

In an installation that already has the Fresnica app passcode, the current terminal restore path can decrypt/verify protected signer identity before accepting it. In an empty installation, v1 may store the encrypted record before every semantic relationship can be independently proven.

A future Mobile/Desktop portable backup must close this gap by integrity-protecting security-significant metadata or by keeping restored records inactive until the required independent validation/reconfirmation succeeds.

## Implementation freedom

The common contract does not require:

- one JSON/CBOR/archive schema;
- one file extension;
- one cloud provider;
- one Account/Signer database layout;
- the terminal v1 single-record shape;
- exporting device-bound System Auth registrations;
- one product recovery UX.

Products may also provide device/cloud backups in addition to portable Fresnica backup, but must describe their portability/security properties accurately.

## Promotion criteria

Promote Backup / Restore only after at least one non-terminal product demonstrates a portable Account/Signer/Recovery graph format and activation/validation lifecycle that can be shared without freezing the terminal v1 record model.

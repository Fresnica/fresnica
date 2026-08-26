# Signing Material Reveal and Export

Status: **accepted pre-release security contract**.

Fresnica normally keeps protected software-signer material inside Rust Core. Signing, transaction authorization, and normal wallet use must not require returning a mnemonic or private key to the mobile JavaScript runtime.

A user may nevertheless need the original signing material for migration, backup verification, or moving to another wallet. Fresnica therefore supports an explicit **Reveal / Export** operation as a controlled declassification boundary.

## Principle

Normal signing and secret export are different security operations.

```text
Normal signing
-------------
protected signer envelope
      |
Core unlocks + validates signer identity + signs
      |
signature / signed XDR

Secret export
-------------
user explicitly requests Reveal / Export
      |
mandatory fresh Fresnica app-passcode entry
      |
Core decrypts + validates signer identity
      |
original secret material crosses the Core boundary temporarily
      |
Mobile displays or exports it under strict handling rules
```

System authentication by itself is not sufficient to reveal signing material. A previously unlocked application session is also not sufficient. Reveal / Export requires fresh app-passcode authentication because the operation permanently changes the confidentiality boundary: a signature authorizes one operation, while disclosure can allow unlimited future signing elsewhere.

## What may be exported

Core returns the original recoverable signing material represented by the protected software-signer payload.

### Mnemonic-backed signer

Export may return:

- the original mnemonic;
- the mnemonic passphrase, if one was used;
- derivation index;
- mnemonic language metadata.

The mnemonic must be returned exactly as stored. Derivation metadata must be sufficient to reconstruct the same signer public key.

### Secret-key-backed signer

Export may return the original Stellar `S...` secret.

A Stellar secret key cannot be converted back into an original mnemonic. If a signer was imported from `S...` and no mnemonic was stored, Fresnica must not imply that a mnemonic can later be recovered.

### External / hardware / remote signer

Fresnica cannot export secret material that it never possessed. Hardware, external, remote, passkey, and similar signers may expose only whatever export mechanism their own provider explicitly supports.

## Required Core checks

Before returning any signing material, Rust Core MUST:

1. require password-based software-signer authentication rather than system-auth-only authorization;
2. decrypt the canonical protected signer envelope;
3. parse the signing-material type;
4. construct or derive the corresponding signer;
5. verify that the resulting public key matches `expected_signer_public_key`;
6. fail closed on unsupported, corrupted, or identity-mismatched material;
7. return only the signing material belonging to the validated signer.

The identity check is mandatory even though the user supplied the passcode. It prevents corrupted or substituted encrypted material from being presented as belonging to another signer/account record.

Account identity and signer identity are separate. For an ordinary master-key wallet the two `G...` values match; for Stellar additional/multisig signers they may differ. Reveal validates the signer represented by the envelope, not ledger authorization for a particular account.

## Mobile handling rules

Once Core returns plaintext signing material, Mobile is temporarily handling a declassified secret and MUST treat that path differently from ordinary wallet state.

Mobile MUST NOT:

- persist the plaintext in Realm, application preferences, files, logs, crash reports, analytics, or telemetry;
- automatically copy the secret or mnemonic to the clipboard;
- cache the revealed value beyond the active export flow;
- include it in screenshots, application state restoration, navigation serialization, or debug traces where the platform permits those to be disabled;
- reuse system-auth session state as the sole authorization for disclosure.

Mobile SHOULD:

- clearly warn that anyone seeing the material can invoke that signer wherever it is authorized;
- require an explicit user action before copying or sharing;
- keep the reveal screen short-lived;
- clear native buffers and UI state as soon as practical when leaving the flow;
- prevent routine background snapshots of the reveal screen where the operating system allows it.

JavaScript strings cannot be reliably zeroized. The preferred long-term mobile integration is therefore a native/Core reveal path that minimizes copies and keeps plaintext in JavaScript only for the shortest UI interval required to show it.

## API boundary

Core exposes secret export separately from signer unlock, re-protection, and transaction signing.

Conceptually:

```text
export_signing_material(
    protected_signer_envelope,
    app_passcode,
    expected_signer_public_key,
) -> ExportedSigningMaterial
```

The export API must not be reused by normal transaction signing or passcode rotation. Normal signing uses `WalletUnlockKey`; passcode rotation uses dedicated Core `reprotect` so plaintext material does not cross the boundary.

## Relation to system authentication

System authentication answers:

> Is this user authorized to invoke this signer now?

Reveal / Export answers a different question:

> Has the user explicitly chosen to remove this signing material from Fresnica's confidentiality boundary?

The first may be authorized by biometrics or another OS mechanism. The second requires fresh Fresnica app-passcode authentication.

This distinction must remain intact for future signer types and platform integrations.

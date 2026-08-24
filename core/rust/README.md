# Fresnica Core (Rust)

This directory is the production Rust Core for Fresnica.

The Python reference remains the behavioral authority while stable semantics are ported. Rust code should reuse established Stellar primitives and reproduce existing cross-language test-vector behavior instead of introducing a parallel wallet model.

## Current scope

Implemented production primitives currently include:

- Classic Stellar account identities (`G...`)
- Contract account identities (`C...`) without contract runtime assumptions
- SEP-0005 deterministic Classic public-key derivation and supported-language detection
- Classic Ed25519 software signer
- External Ed25519 transaction signer for hardware, device, or process-backed providers
- Classic transaction envelope hashing and decorated-signature attachment through the official `stellar-xdr` crate
- Password and system-key protection providers for locally stored wallet signing material
- Protected signing-material unlock into a Classic `SoftwareSigner` with public-key identity validation

Transaction building, network submission, storage, SDEX, anchors, Soroban account authorization, passkeys, and UI remain outside the current Rust Core slice.

## Signing boundary

Classic transaction signing signs an exact 32-byte Stellar transaction hash. `TransactionSigningRequest` also carries the current raw envelope XDR and network passphrase so an external signer can inspect public transaction context before approving a signature; those fields are review context, not an alternate payload to sign.

External signers hold only the declared Stellar public key and a provider callback. Fresnica verifies the provider's returned Ed25519 signature against the exact transaction hash before mutating the envelope. Private signing material remains outside Fresnica for hardware/device/process-backed signers.

Arbitrary message signing is reserved as a separate future capability following **SEP-53 (Sign and Verify Messages)**. That extension must preserve SEP-53 domain separation (`Stellar Signed Message:\n`) rather than widening the transaction-signing method to accept arbitrary bytes.

## Secret-protection boundary

Protection applies only to secret material held locally for software signing. Hardware, device, remote, and future contract-account signers do not route private keys through `ProtectionProvider`.

Password protection preserves the existing version-1 Scrypt + AES-256-GCM wallet format. System protection generates a random 32-byte wrapping key and delegates only that key to `SystemKeyStore`, which is the platform boundary for Keychain, DPAPI/Hello, Android Keystore, or another OS facility. Fresnica persists the encrypted wallet payload plus an opaque key reference, not the wrapping key itself.

Password-derived keys, system wrapping keys, loaded system keys, and intermediate decrypted byte buffers use zeroizing containers. `unlock_software_signer` consumes decrypted `secret` or `mnemonic` strings by moving their allocations into zeroizing containers before constructing the signer, then verifies that the resulting Stellar public key matches public wallet metadata. Generic decoded payloads remain live plaintext and should not be cached. No global vault or master key is introduced by this layer.

## Validation

```sh
cargo test --manifest-path core/rust/Cargo.toml
```

Future slices should consume `spec/test-vectors` where a stable language-neutral contract already exists.

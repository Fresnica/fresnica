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
- One-shot protected transaction signing that keeps secret material inside Core
- Explicit password-only signing-material export for user-requested reveal/migration flows
- Agent Access capability checks before Classic transaction signing

Transaction building, network submission, storage, SDEX, anchors, Soroban account authorization, passkeys, and UI remain outside the current Rust Core slice.

## Signing boundary

Classic transaction signing signs an exact 32-byte Stellar transaction hash. `TransactionSigningRequest` also carries the current raw envelope XDR and network passphrase so an external signer can inspect public transaction context before approving a signature; those fields are review context, not an alternate payload to sign.

External signers hold only the declared Stellar public key and a provider callback. Fresnica verifies the provider's returned Ed25519 signature against the exact transaction hash before mutating the envelope. Private signing material remains outside Fresnica for hardware/device/process-backed signers.

`sign_protected_transaction_envelope` is the preferred foundation for mobile software-wallet signing: it unlocks protected material, verifies the expected public identity, signs the transaction, and drops the secret-bearing signer without returning a private key to the caller.

Arbitrary message signing is reserved as a separate future capability following **SEP-53 (Sign and Verify Messages)**. That extension must preserve SEP-53 domain separation (`Stellar Signed Message:\n`) rather than widening the transaction-signing method to accept arbitrary bytes.

## Secret-protection boundary

Protection applies only to secret material held locally for software signing. Hardware, device, remote, and future contract-account signers do not route private keys through local wallet protection.

Password protection preserves the version-1 Scrypt + AES-256-GCM wallet format. Each protected wallet envelope carries independent random KDF salt and AEAD nonce material, so the product may use one Fresnica app passcode while wallets still receive different effective encryption keys.

The current Rust implementation also contains `SystemProtectionProvider` / `SystemKeyStore`, which generates a separate random wallet protection key and stores it through an injected platform key-store boundary. This remains valid prototype code and test coverage, but it is **not the target mobile product model** after the Mobile/Core vault contract was accepted.

For mobile integration, system authentication is signer authorization rather than a second wallet encryption format. Keychain / Keystore, biometrics, app lock/session state, Realm/database encryption, and persistence belong to the mobile layer. Mobile persists Core-generated protected wallet envelopes as opaque data. A system-auth shortcut for a software signer must authorize unlocking the same canonical Core envelope used by manual app-passcode entry; it must not create a second independently encrypted wallet payload.

Password-derived keys, system prototype keys, and intermediate decrypted byte buffers use zeroizing containers. `unlock_software_signer` consumes decrypted `secret` or `mnemonic` strings by moving their allocations into zeroizing containers before constructing the signer, then verifies that the resulting Stellar public key matches public wallet metadata. Generic decoded payloads remain live plaintext and should not be cached.

`export_signing_material` is deliberately separate from normal signing. It requires a password credential, reconstructs and validates the signer identity before returning material, and returns either the stored Stellar secret or the stored mnemonic plus passphrase/derivation metadata. System authorization alone cannot use this API. Exported values use zeroizing containers and redact their `Debug` representation, but any caller that intentionally reveals them must still treat the plaintext as declassified secret data.

Before mobile FFI is frozen, Core still needs to decouple signer/system authorization from the registry's mutually exclusive `ProtectionCredential::System` path and define the native system-auth unlock credential contract with the mobile layer.

See [`docs/mobile-core-contract.md`](../../docs/mobile-core-contract.md), [`docs/protection.md`](../../docs/protection.md), and [`docs/secret-export.md`](../../docs/secret-export.md).

## Agent Access boundary

Agent Access authorizes use of an existing signer; it does not give an agent wallet secret material. `AgentCapability` is public policy data binding a Classic G account to one network, an explicit Stellar `OperationType` allowlist, maximum operation count, total transaction-fee ceiling, and optional expiry.

The first policy slice is deliberately fail-closed: only unsigned Classic V1 envelopes are accepted; the transaction source and every effective operation source must resolve to the capability's G account; V0 and fee-bump envelopes are rejected; unlisted operations, excessive fees/counts, expired capabilities, and signer/account mismatches are rejected before signing. `sign_agent_transaction` runs authorization and signing in the same call so callers cannot authorize one envelope and then substitute another before the signer is invoked.

This is an **operation-level foundation**, not the finished autonomous-spending policy. Allowing `PAYMENT`, SDEX, trustline, sponsorship, or Soroban operation types currently grants that operation type without destination, asset, amount, market, contract, or argument constraints. Those constraints must be added before product adapters expose corresponding broad capabilities. Token issuance/storage, MCP, CLI, local RPC/daemon, and OWS-compatible transports remain adapter-layer work and must not duplicate the authorization path.

## Validation

```sh
cargo test --manifest-path core/rust/Cargo.toml
```

Future slices should consume `spec/test-vectors` where a stable language-neutral contract already exists.

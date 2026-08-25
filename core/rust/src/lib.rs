//! Fresnica production core.
//!
//! The Rust core ports stable semantics from the Python reference. It should not
//! invent parallel wallet behavior.

pub mod account;
pub mod agent_access;
pub mod client_api;
pub mod protected_signer;
pub mod protection;
pub mod secret_store;
pub mod signer;
pub mod transaction;
pub mod wallet;
pub mod wallet_material;

pub use account::{AccountError, AccountIdentity, AccountKind};
pub use agent_access::{
    authorize_agent_transaction, sign_agent_transaction, AgentAccessError, AgentAuthorization,
    AgentCapability,
};
pub use client_api::{
    ClientAccountIdentity, ClientAccountKind, ClientApiError, ClientApiErrorCode,
    ClientEd25519SigningRequest, ClientGeneratedMnemonic, ClientProtectedSoftwareSigner,
    CoreClientApi, CLIENT_API_VERSION,
};
pub use protected_signer::{
    derive_verified_unlock_key, export_signing_material, sign_protected_transaction_envelope,
    unlock_software_signer, ExportedSigningMaterial, ProtectedSignerError, ProtectedSigningError,
};
pub use protection::{
    PasswordProtectionProvider, ProtectionCredential, ProtectionError, ProtectionProvider,
    ProtectionRegistry, PROTECTED_SECRET_FORMAT, PROTECTED_SECRET_VERSION,
};
pub use secret_store::{
    decrypt_secret, decrypt_secret_with_unlock_key, derive_unlock_key, encrypt_secret,
    PasswordSecretEnvelope, ScryptEnvelope, SecretStoreError, WalletUnlockKey, SCRYPT_N, SCRYPT_P,
    SCRYPT_R,
};
pub use signer::{
    ClassicSigner, ExternalEd25519Signer, SignerError, SoftwareSigner,
    TransactionSigningRequest,
};
pub use transaction::{
    network_id, parse_transaction_envelope_xdr, sign_transaction_envelope,
    transaction_envelope_xdr, transaction_hash, verify_transaction_envelope_signature,
    TransactionSigningError,
};
pub use wallet::{
    derive_classic_public_key, derive_classic_signer, detect_mnemonic_language,
    generate_mnemonic_phrase, WalletDerivationError,
};
pub use wallet_material::{
    generate_protected_mnemonic, protect_mnemonic_signing_material,
    protect_secret_signing_material, GeneratedProtectedMnemonic, ProtectedWalletMaterial,
    WalletMaterialError,
};

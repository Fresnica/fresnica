//! Fresnica production core.
//!
//! The Rust core ports stable semantics from the Python reference. It should not
//! invent parallel wallet behavior.

pub mod account;
pub mod agent_access;
pub mod protected_signer;
pub mod protection;
pub mod secret_store;
pub mod signer;
pub mod transaction;
pub mod wallet;

pub use account::{AccountError, AccountIdentity, AccountKind};
pub use agent_access::{
    authorize_agent_transaction, sign_agent_transaction, AgentAccessError, AgentAuthorization,
    AgentCapability,
};
pub use protected_signer::{unlock_software_signer, ProtectedSignerError};
pub use protection::{
    PasswordProtectionProvider, ProtectionCredential, ProtectionError, ProtectionProvider,
    ProtectionRegistry, SystemKeyStore, SystemKeyStoreError, SystemProtectionProvider,
    PROTECTED_SECRET_FORMAT, PROTECTED_SECRET_VERSION,
};
pub use secret_store::{
    decrypt_secret, decrypt_secret_with_key, encrypt_secret, encrypt_secret_with_key,
    KeySecretEnvelope, PasswordSecretEnvelope, ScryptEnvelope, SecretStoreError, SCRYPT_N,
    SCRYPT_P, SCRYPT_R,
};
pub use signer::{
    ClassicSigner, ExternalEd25519Signer, SignerError, SoftwareSigner,
    TransactionSigningRequest,
};
pub use transaction::{
    network_id, parse_transaction_envelope_xdr, sign_transaction_envelope,
    transaction_envelope_xdr, transaction_hash, TransactionSigningError,
};
pub use wallet::{
    derive_classic_public_key, derive_classic_signer, detect_mnemonic_language,
    WalletDerivationError,
};

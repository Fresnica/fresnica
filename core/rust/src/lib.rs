//! Fresnica production core.
//!
//! The Rust core ports stable semantics from the Python reference. It should not
//! invent parallel wallet behavior.

pub mod account;
pub mod signer;
pub mod transaction;
pub mod wallet;

pub use account::{AccountError, AccountIdentity, AccountKind};
pub use signer::{ClassicSigner, SignerError, SoftwareSigner};
pub use transaction::{
    network_id, parse_transaction_envelope_xdr, sign_transaction_envelope,
    transaction_envelope_xdr, transaction_hash, TransactionSigningError,
};
pub use wallet::{derive_classic_public_key, WalletDerivationError};

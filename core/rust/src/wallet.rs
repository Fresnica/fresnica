use bip39::{Language, Mnemonic};
use kobe_primitives::slip10::DerivedEd25519Key;
use stellar_strkey::ed25519::PublicKey;
use thiserror::Error;

const STELLAR_PURPOSE: u32 = 44;
const STELLAR_COIN_TYPE: u32 = 148;
const MAX_ACCOUNT_INDEX: usize = (1 << 31) - 1;

#[derive(Debug, Error)]
pub enum WalletDerivationError {
    #[error("unsupported mnemonic language: {0}")]
    UnsupportedLanguage(String),
    #[error("invalid Stellar mnemonic phrase")]
    InvalidMnemonic,
    #[error("invalid Stellar account index")]
    InvalidIndex,
}

fn mnemonic_language(language: &str) -> Result<Language, WalletDerivationError> {
    match language {
        "english" => Ok(Language::English),
        "chinese-simplified" => Ok(Language::SimplifiedChinese),
        "chinese-traditional" => Ok(Language::TraditionalChinese),
        "french" => Ok(Language::French),
        "italian" => Ok(Language::Italian),
        "japanese" => Ok(Language::Japanese),
        "korean" => Ok(Language::Korean),
        "spanish" => Ok(Language::Spanish),
        other => Err(WalletDerivationError::UnsupportedLanguage(other.to_owned())),
    }
}

fn derive_key(
    mnemonic: &str,
    passphrase: &str,
    index: usize,
    language: &str,
) -> Result<DerivedEd25519Key, WalletDerivationError> {
    if index > MAX_ACCOUNT_INDEX {
        return Err(WalletDerivationError::InvalidIndex);
    }

    let mnemonic = Mnemonic::parse_in(mnemonic_language(language)?, mnemonic.trim())
        .map_err(|_| WalletDerivationError::InvalidMnemonic)?;
    let seed = mnemonic.to_seed(passphrase);

    DerivedEd25519Key::derive_path(
        &seed,
        &format!("m/{STELLAR_PURPOSE}'/{STELLAR_COIN_TYPE}'/{index}'"),
    )
    .map_err(|_| WalletDerivationError::InvalidIndex)
}

pub fn derive_classic_public_key(
    mnemonic: &str,
    passphrase: &str,
    index: usize,
    language: &str,
) -> Result<String, WalletDerivationError> {
    Ok(format!("{}", PublicKey(derive_key(mnemonic, passphrase, index, language)?.public_key_bytes())))
}

pub fn derive_classic_signer(
    mnemonic: &str,
    passphrase: &str,
    index: usize,
    language: &str,
) -> Result<crate::SoftwareSigner, WalletDerivationError> {
    crate::SoftwareSigner::from_signing_key(
        derive_key(mnemonic, passphrase, index, language)?.to_signing_key(),
    )
    .map_err(|_| WalletDerivationError::InvalidIndex)
}

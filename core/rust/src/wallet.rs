use bip39::{Language, Mnemonic};
use kobe_primitives::slip10::DerivedEd25519Key;
use stellar_strkey::ed25519::PublicKey;
use thiserror::Error;
use zeroize::Zeroizing;

const STELLAR_PURPOSE: u32 = 44;
const STELLAR_COIN_TYPE: u32 = 148;
const MAX_ACCOUNT_INDEX: usize = (1 << 31) - 1;
const SUPPORTED_MNEMONIC_LANGUAGES: &[(&str, Language)] = &[
    ("english", Language::English),
    ("chinese_simplified", Language::SimplifiedChinese),
    ("chinese_traditional", Language::TraditionalChinese),
    ("french", Language::French),
    ("italian", Language::Italian),
    ("japanese", Language::Japanese),
    ("korean", Language::Korean),
    ("spanish", Language::Spanish),
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WalletDerivationError {
    #[error("unsupported mnemonic language: {0}")]
    UnsupportedLanguage(String),
    #[error("invalid Stellar mnemonic phrase")]
    InvalidMnemonic,
    #[error("mnemonic language is ambiguous; specify it explicitly")]
    AmbiguousLanguage,
    #[error("invalid Stellar account index")]
    InvalidIndex,
}

fn mnemonic_language(language: &str) -> Result<Language, WalletDerivationError> {
    SUPPORTED_MNEMONIC_LANGUAGES
        .iter()
        .find_map(|(name, value)| (*name == language).then_some(*value))
        .ok_or_else(|| WalletDerivationError::UnsupportedLanguage(language.to_owned()))
}

pub fn detect_mnemonic_language(
    mnemonic: &str,
) -> Result<&'static str, WalletDerivationError> {
    let mnemonic = mnemonic.trim();
    let mut detected = None;

    for &(name, language) in SUPPORTED_MNEMONIC_LANGUAGES {
        if Mnemonic::parse_in(language, mnemonic).is_ok() {
            if detected.is_some() {
                return Err(WalletDerivationError::AmbiguousLanguage);
            }
            detected = Some(name);
        }
    }

    detected.ok_or(WalletDerivationError::InvalidMnemonic)
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
    let seed = Zeroizing::new(mnemonic.to_seed(passphrase));

    DerivedEd25519Key::derive_path(
        &seed[..],
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
    Ok(format!(
        "{}",
        PublicKey(derive_key(mnemonic, passphrase, index, language)?.public_key_bytes())
    ))
}

pub fn derive_classic_signer(
    mnemonic: &str,
    passphrase: &str,
    index: usize,
    language: &str,
) -> Result<crate::SoftwareSigner, WalletDerivationError> {
    Ok(crate::SoftwareSigner::from_signing_key(
        derive_key(mnemonic, passphrase, index, language)?.to_signing_key(),
    ))
}

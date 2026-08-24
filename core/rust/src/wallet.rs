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
    #[error("invalid mnemonic entropy strength")]
    InvalidStrength,
    #[error("secure randomness is unavailable")]
    RandomUnavailable,
}

fn mnemonic_language(language: &str) -> Result<Language, WalletDerivationError> {
    SUPPORTED_MNEMONIC_LANGUAGES
        .iter()
        .find_map(|(name, value)| (*name == language).then_some(*value))
        .ok_or_else(|| WalletDerivationError::UnsupportedLanguage(language.to_owned()))
}

pub fn generate_mnemonic_phrase(
    language: &str,
    strength: usize,
) -> Result<Zeroizing<String>, WalletDerivationError> {
    let entropy_bytes = match strength {
        128 => 16,
        160 => 20,
        192 => 24,
        224 => 28,
        256 => 32,
        _ => return Err(WalletDerivationError::InvalidStrength),
    };
    let mut entropy = Zeroizing::new(vec![0u8; entropy_bytes]);
    getrandom::fill(&mut entropy[..]).map_err(|_| WalletDerivationError::RandomUnavailable)?;
    let mnemonic = Mnemonic::from_entropy_in(mnemonic_language(language)?, &entropy)
        .map_err(|_| WalletDerivationError::InvalidStrength)?;
    Ok(Zeroizing::new(mnemonic.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_mnemonic_roundtrips_through_derivation() {
        let mnemonic = generate_mnemonic_phrase("english", 128).unwrap();
        assert_eq!(mnemonic.split_whitespace().count(), 12);
        assert!(derive_classic_public_key(&mnemonic, "", 0, "english").is_ok());
    }

    #[test]
    fn rejects_non_bip39_entropy_strength() {
        assert_eq!(
            generate_mnemonic_phrase("english", 129).unwrap_err(),
            WalletDerivationError::InvalidStrength
        );
    }
}

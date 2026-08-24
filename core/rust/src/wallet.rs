use sep5::SeedPhrase;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WalletDerivationError {
    #[error("unsupported mnemonic language: {0}")]
    UnsupportedLanguage(String),
    #[error("invalid Stellar mnemonic phrase")]
    InvalidMnemonic,
    #[error("invalid Stellar account index")]
    InvalidIndex,
}

pub fn derive_classic_public_key(
    mnemonic: &str,
    passphrase: &str,
    index: usize,
    language: &str,
) -> Result<String, WalletDerivationError> {
    if language != "english" {
        return Err(WalletDerivationError::UnsupportedLanguage(
            language.to_owned(),
        ));
    }

    let seed = SeedPhrase::from_seed_phrase(mnemonic)
        .map_err(|_| WalletDerivationError::InvalidMnemonic)?;
    let keypair = seed
        .from_path_index(index, Some(passphrase))
        .map_err(|_| WalletDerivationError::InvalidIndex)?;

    Ok(format!("{}", keypair.public()))
}

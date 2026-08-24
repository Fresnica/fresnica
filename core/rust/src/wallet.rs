use std::str::FromStr;

use near_slip10::{derive_key_from_mnemonic, BIP32Path};
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
    if index > MAX_ACCOUNT_INDEX {
        return Err(WalletDerivationError::InvalidIndex);
    }

    let path = BIP32Path::from_str(&format!(
        "m/{STELLAR_PURPOSE}'/{STELLAR_COIN_TYPE}'/{index}'"
    ))
    .map_err(|_| WalletDerivationError::InvalidIndex)?;
    let key = derive_key_from_mnemonic(mnemonic.trim(), passphrase, &path).map_err(|error| {
        match error {
            near_slip10::MnemonicError::InvalidMnemonic(_) => {
                WalletDerivationError::InvalidMnemonic
            }
            near_slip10::MnemonicError::Derivation(_) => WalletDerivationError::InvalidIndex,
        }
    })?;

    let public = key.public_key();
    let public_bytes: [u8; 32] = public[1..]
        .try_into()
        .expect("SLIP-10 Ed25519 public keys are 33 bytes with a one-byte prefix");
    Ok(format!("{}", PublicKey(public_bytes)))
}

use ed25519_dalek::{Signer as _, SigningKey};
use stellar_strkey::ed25519::{PrivateKey, PublicKey};
use thiserror::Error;

pub trait ClassicSigner {
    fn public_key(&self) -> &str;
    fn sign_transaction_hash(&self, transaction_hash: &[u8; 32]) -> [u8; 64];

    fn signature_hint(&self) -> [u8; 4] {
        let public = PublicKey::from_string(self.public_key())
            .expect("ClassicSigner public keys must be valid Stellar Ed25519 keys");
        public.0[28..32]
            .try_into()
            .expect("Stellar Ed25519 public keys are 32 bytes")
    }
}

pub struct SoftwareSigner {
    signing_key: SigningKey,
    public_key: String,
}

impl SoftwareSigner {
    pub fn from_secret(secret: &str) -> Result<Self, SignerError> {
        let private =
            PrivateKey::from_string(secret.trim()).map_err(|_| SignerError::InvalidSecret)?;
        Ok(Self::from_signing_key(SigningKey::from_bytes(&private.0)))
    }

    pub(crate) fn from_signing_key(signing_key: SigningKey) -> Self {
        let public_key = format!("{}", PublicKey(signing_key.verifying_key().to_bytes()));
        Self {
            signing_key,
            public_key,
        }
    }
}

impl ClassicSigner for SoftwareSigner {
    fn public_key(&self) -> &str {
        &self.public_key
    }

    fn sign_transaction_hash(&self, transaction_hash: &[u8; 32]) -> [u8; 64] {
        self.signing_key.sign(transaction_hash).to_bytes()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignerError {
    #[error("invalid Stellar secret key")]
    InvalidSecret,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    fn decode_hex<const N: usize>(hex: &str) -> [u8; N] {
        assert_eq!(hex.len(), N * 2);
        let mut out = [0u8; N];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn parses_stellar_secret_and_exposes_matching_public_key() {
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        assert_eq!(signer.public_key(), PUBLIC);
        assert_eq!(signer.signature_hint(), [0xf7, 0x07, 0x51, 0x1a]);
    }

    #[test]
    fn signs_exact_transaction_hash() {
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        let transaction_hash = core::array::from_fn(|index| index as u8);
        let expected = decode_hex::<64>(concat!(
            "00c1db988bb12fd7351a6054ae3fac90fab7e4fc56b1651c7181f5f55f896f66",
            "3933d3a90605d9058e9d0ac45950ee2d3c9c9b14857415587179fe0ccac35f09"
        ));
        assert_eq!(signer.sign_transaction_hash(&transaction_hash), expected);
    }

    #[test]
    fn rejects_non_secret_strkey() {
        assert_eq!(
            SoftwareSigner::from_secret(PUBLIC).err(),
            Some(SignerError::InvalidSecret)
        );
    }
}

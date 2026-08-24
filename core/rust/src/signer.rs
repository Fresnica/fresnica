use ed25519_dalek::{Signer as _, SigningKey};
use stellar_strkey::ed25519::{PrivateKey, PublicKey};
use thiserror::Error;

pub trait ClassicSigner {
    fn public_key(&self) -> &str;
    fn sign_payload(&self, payload: &[u8]) -> [u8; 64];

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
        let signing_key = SigningKey::from_bytes(&private.0);
        let public_key = format!("{}", PublicKey(signing_key.verifying_key().to_bytes()));

        Ok(Self {
            signing_key,
            public_key,
        })
    }
}

impl ClassicSigner for SoftwareSigner {
    fn public_key(&self) -> &str {
        &self.public_key
    }

    fn sign_payload(&self, payload: &[u8]) -> [u8; 64] {
        self.signing_key.sign(payload).to_bytes()
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

    // RFC 8032 test vector 1 encoded as a Stellar secret/public key pair.
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
    fn signs_exact_ed25519_payload() {
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        let expected = decode_hex::<64>(concat!(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155",
            "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        ));

        assert_eq!(signer.sign_payload(b""), expected);
    }

    #[test]
    fn rejects_non_secret_strkey() {
        assert_eq!(
            SoftwareSigner::from_secret(PUBLIC).err(),
            Some(SignerError::InvalidSecret)
        );
    }
}

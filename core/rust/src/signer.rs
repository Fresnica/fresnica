use ed25519_dalek::{Signer as _, SigningKey};
use stellar_strkey::ed25519::{PrivateKey, PublicKey};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionSigningRequest {
    pub transaction_hash: [u8; 32],
    pub transaction_xdr: Vec<u8>,
    pub network_passphrase: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SorobanAuthorizationSigningRequest {
    pub authorization_hash: [u8; 32],
    pub authorization_entry_xdr: Vec<u8>,
    pub authorization_preimage_xdr: Vec<u8>,
    pub network_passphrase: String,
}

pub trait ClassicSigner {
    fn public_key(&self) -> &str;
    fn sign_transaction(
        &self,
        request: &TransactionSigningRequest,
    ) -> Result<[u8; 64], SignerError>;

    fn signature_hint(&self) -> Result<[u8; 4], SignerError> {
        let public =
            PublicKey::from_string(self.public_key()).map_err(|_| SignerError::InvalidPublicKey)?;
        Ok(public.0[28..32]
            .try_into()
            .expect("Stellar Ed25519 public keys are 32 bytes"))
    }
}

pub trait SorobanAuthorizationSigner {
    fn signer_public_key(&self) -> &str;
    fn sign_soroban_authorization(
        &self,
        request: &SorobanAuthorizationSigningRequest,
    ) -> Result<[u8; 64], SignerError>;
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

    fn sign_transaction(
        &self,
        request: &TransactionSigningRequest,
    ) -> Result<[u8; 64], SignerError> {
        Ok(self.signing_key.sign(&request.transaction_hash).to_bytes())
    }
}

impl SorobanAuthorizationSigner for SoftwareSigner {
    fn signer_public_key(&self) -> &str {
        &self.public_key
    }

    fn sign_soroban_authorization(
        &self,
        request: &SorobanAuthorizationSigningRequest,
    ) -> Result<[u8; 64], SignerError> {
        Ok(self
            .signing_key
            .sign(&request.authorization_hash)
            .to_bytes())
    }
}

type ExternalSigningProvider = dyn Fn(&TransactionSigningRequest) -> Result<[u8; 64], SignerError>;

pub struct ExternalEd25519Signer {
    public_key: String,
    sign_request: Box<ExternalSigningProvider>,
}

impl ExternalEd25519Signer {
    pub fn new<F>(public_key: &str, sign_request: F) -> Result<Self, SignerError>
    where
        F: Fn(&TransactionSigningRequest) -> Result<[u8; 64], SignerError> + 'static,
    {
        let public =
            PublicKey::from_string(public_key.trim()).map_err(|_| SignerError::InvalidPublicKey)?;
        Ok(Self {
            public_key: format!("{public}"),
            sign_request: Box::new(sign_request),
        })
    }
}

impl ClassicSigner for ExternalEd25519Signer {
    fn public_key(&self) -> &str {
        &self.public_key
    }

    fn sign_transaction(
        &self,
        request: &TransactionSigningRequest,
    ) -> Result<[u8; 64], SignerError> {
        (self.sign_request)(request)
    }
}

type ExternalSorobanAuthorizationSigningProvider =
    dyn Fn(&SorobanAuthorizationSigningRequest) -> Result<[u8; 64], SignerError>;

pub struct ExternalSorobanEd25519Signer {
    public_key: String,
    sign_request: Box<ExternalSorobanAuthorizationSigningProvider>,
}

impl ExternalSorobanEd25519Signer {
    pub fn new<F>(public_key: &str, sign_request: F) -> Result<Self, SignerError>
    where
        F: Fn(&SorobanAuthorizationSigningRequest) -> Result<[u8; 64], SignerError> + 'static,
    {
        let public =
            PublicKey::from_string(public_key.trim()).map_err(|_| SignerError::InvalidPublicKey)?;
        Ok(Self {
            public_key: format!("{public}"),
            sign_request: Box::new(sign_request),
        })
    }
}

impl SorobanAuthorizationSigner for ExternalSorobanEd25519Signer {
    fn signer_public_key(&self) -> &str {
        &self.public_key
    }

    fn sign_soroban_authorization(
        &self,
        request: &SorobanAuthorizationSigningRequest,
    ) -> Result<[u8; 64], SignerError> {
        (self.sign_request)(request)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignerError {
    #[error("invalid Stellar secret key")]
    InvalidSecret,
    #[error("invalid Stellar public key")]
    InvalidPublicKey,
    #[error("external signer failed: {0}")]
    ExternalProvider(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 8032 test-vector keypair encoded as Stellar StrKeys.
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

    fn request(transaction_hash: [u8; 32]) -> TransactionSigningRequest {
        TransactionSigningRequest {
            transaction_hash,
            transaction_xdr: Vec::new(),
            network_passphrase: String::new(),
        }
    }

    #[test]
    fn parses_stellar_secret_and_exposes_matching_public_key() {
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();

        assert_eq!(signer.public_key(), PUBLIC);
        assert_eq!(signer.signature_hint().unwrap(), [0xf7, 0x07, 0x51, 0x1a]);
    }

    #[test]
    fn signs_exact_transaction_hash() {
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        let transaction_hash = core::array::from_fn(|index| index as u8);
        let expected = decode_hex::<64>(concat!(
            "00c1db988bb12fd7351a6054ae3fac90fab7e4fc56b1651c7181f5f55f896f66",
            "3933d3a90605d9058e9d0ac45950ee2d3c9c9b14857415587179fe0ccac35f09"
        ));

        assert_eq!(
            signer.sign_transaction(&request(transaction_hash)).unwrap(),
            expected
        );
    }

    #[test]
    fn rejects_non_secret_strkey() {
        assert_eq!(
            SoftwareSigner::from_secret(PUBLIC).err(),
            Some(SignerError::InvalidSecret)
        );
    }

    #[test]
    fn external_signer_rejects_invalid_public_key() {
        let signer = ExternalEd25519Signer::new("not-a-stellar-key", |_| Ok([0u8; 64]));

        assert_eq!(signer.err(), Some(SignerError::InvalidPublicKey));
    }
}

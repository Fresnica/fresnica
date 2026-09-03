use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::PublicKey;
use thiserror::Error;

use crate::signer::{MessageSigner, MessageSigningRequest, SignerError};

pub const SEP53_MESSAGE_PREFIX: &[u8] = b"Stellar Signed Message:\n";

pub fn sep53_message_payload(message: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(SEP53_MESSAGE_PREFIX.len() + message.len());
    payload.extend_from_slice(SEP53_MESSAGE_PREFIX);
    payload.extend_from_slice(message);
    payload
}

pub fn sep53_message_hash(message: &[u8]) -> [u8; 32] {
    Sha256::digest(sep53_message_payload(message)).into()
}

pub fn prepare_message_signing(message: &[u8]) -> MessageSigningRequest {
    let encoded_message = sep53_message_payload(message);
    let message_hash = Sha256::digest(&encoded_message).into();
    MessageSigningRequest {
        message: message.to_vec(),
        encoded_message,
        message_hash,
    }
}

pub fn sign_message<S: MessageSigner + ?Sized>(
    message: &[u8],
    signer: &S,
) -> Result<[u8; 64], MessageSigningError> {
    let request = prepare_message_signing(message);
    let signature = signer.sign_message(&request)?;
    verify_message_signature(signer.public_key(), message, &signature)?;
    Ok(signature)
}

pub fn verify_message_signature(
    signer_public_key: &str,
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), MessageSigningError> {
    let public =
        PublicKey::from_string(signer_public_key).map_err(|_| SignerError::InvalidPublicKey)?;
    let verifying_key =
        VerifyingKey::from_bytes(&public.0).map_err(|_| SignerError::InvalidPublicKey)?;
    let message_hash = sep53_message_hash(message);
    let signature = Ed25519Signature::from_bytes(signature);
    verifying_key
        .verify_strict(&message_hash, &signature)
        .map_err(|_| MessageSigningError::InvalidSignature)
}

#[derive(Debug, Error)]
pub enum MessageSigningError {
    #[error(transparent)]
    Signer(#[from] SignerError),
    #[error("signer returned an invalid SEP-53 message signature")]
    InvalidSignature,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ed25519_dalek::{Signer as _, SigningKey};
    use stellar_strkey::ed25519::PrivateKey;

    use super::*;
    use crate::{ExternalMessageEd25519Signer, SoftwareSigner};

    const SECRET: &str = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";
    const PUBLIC: &str = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";

    fn decode_hex<const N: usize>(hex: &str) -> [u8; N] {
        assert_eq!(hex.len(), N * 2);
        let mut out = [0u8; N];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn sep53_ascii_example_matches_final_standard() {
        let message = b"Hello, World!";
        assert_eq!(
            sep53_message_hash(message),
            decode_hex::<32>("d52eb59c06bb510d065997ff93077068eed0a486c20215b5e02e1ab0d2ebea5f")
        );

        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        let signature = sign_message(message, &signer).unwrap();
        assert_eq!(
            signature,
            decode_hex::<64>(concat!(
                "7cee5d6d885752104c85eea421dfdcb95abf01f1271d11c4bec3fcbd7874dccd",
                "6e2e98b97b8eb23b643cac4073bb77de5d07b0710139180ae9f3cbba78f2ba04"
            ))
        );
        verify_message_signature(PUBLIC, message, &signature).unwrap();
    }

    #[test]
    fn binary_messages_are_not_reencoded() {
        let message = [0xff, 0x00, 0x80, 0x41];
        let payload = sep53_message_payload(&message);

        assert_eq!(&payload[..SEP53_MESSAGE_PREFIX.len()], SEP53_MESSAGE_PREFIX);
        assert_eq!(&payload[SEP53_MESSAGE_PREFIX.len()..], message);
    }

    #[test]
    fn external_signer_receives_message_payload_and_hash() {
        let message = b"dapp challenge";
        let software = SoftwareSigner::from_secret(SECRET).unwrap();
        let expected = prepare_message_signing(message);
        let captured = Arc::new(Mutex::new(None));
        let capture = Arc::clone(&captured);
        let signer = ExternalMessageEd25519Signer::new(PUBLIC, move |request| {
            *capture.lock().unwrap() = Some(request.clone());
            software.sign_message(request)
        })
        .unwrap();

        let signature = sign_message(message, &signer).unwrap();

        assert_eq!(captured.lock().unwrap().as_ref(), Some(&expected));
        verify_message_signature(PUBLIC, message, &signature).unwrap();
    }

    #[test]
    fn invalid_external_signature_is_rejected() {
        let signer = ExternalMessageEd25519Signer::new(PUBLIC, |_| Ok([0u8; 64])).unwrap();
        assert!(matches!(
            sign_message(b"challenge", &signer),
            Err(MessageSigningError::InvalidSignature)
        ));
    }

    #[test]
    fn raw_ed25519_without_sep53_prefix_is_not_accepted() {
        let private = PrivateKey::from_string(SECRET).unwrap();
        let signing_key = SigningKey::from_bytes(&private.0);
        let message = b"Hello, World!";
        let raw_signature = signing_key.sign(&Sha256::digest(message)).to_bytes();

        assert!(matches!(
            verify_message_signature(PUBLIC, message, &raw_signature),
            Err(MessageSigningError::InvalidSignature)
        ));
    }
}

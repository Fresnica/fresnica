use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::PublicKey;
use stellar_xdr::{
    DecoratedSignature, Limits, ReadXdr, Signature, SignatureHint, TransactionEnvelope, WriteXdr,
};
use thiserror::Error;

use crate::signer::{ClassicSigner, SignerError, TransactionSigningRequest};

pub fn network_id(network_passphrase: &str) -> [u8; 32] {
    Sha256::digest(network_passphrase.as_bytes()).into()
}

pub fn transaction_hash(
    envelope: &TransactionEnvelope,
    network_passphrase: &str,
) -> Result<[u8; 32], TransactionSigningError> {
    envelope
        .hash(network_id(network_passphrase))
        .map_err(TransactionSigningError::Xdr)
}

pub fn sign_transaction_envelope<S: ClassicSigner + ?Sized>(
    envelope: &mut TransactionEnvelope,
    network_passphrase: &str,
    signer: &S,
) -> Result<(), TransactionSigningError> {
    let hash = transaction_hash(envelope, network_passphrase)?;
    let request = TransactionSigningRequest {
        transaction_hash: hash,
        transaction_xdr: transaction_envelope_xdr(envelope)?,
        network_passphrase: network_passphrase.to_owned(),
    };
    let signature_bytes = signer.sign_transaction(&request)?;
    verify_signature(signer.public_key(), &hash, &signature_bytes)?;

    let decorated = DecoratedSignature {
        hint: SignatureHint(signer.signature_hint()?),
        signature: Signature(
            signature_bytes
                .to_vec()
                .try_into()
                .map_err(TransactionSigningError::Xdr)?,
        ),
    };

    let signatures = match envelope {
        TransactionEnvelope::TxV0(value) => &mut value.signatures,
        TransactionEnvelope::Tx(value) => &mut value.signatures,
        TransactionEnvelope::TxFeeBump(value) => &mut value.signatures,
    };

    if signatures.iter().any(|existing| existing == &decorated) {
        return Err(TransactionSigningError::DuplicateSignature);
    }

    let mut updated: Vec<_> = signatures.clone().into();
    updated.push(decorated);
    *signatures = updated
        .try_into()
        .map_err(TransactionSigningError::Xdr)?;
    Ok(())
}

fn verify_signature(
    public_key: &str,
    transaction_hash: &[u8; 32],
    signature_bytes: &[u8; 64],
) -> Result<(), TransactionSigningError> {
    let public = PublicKey::from_string(public_key).map_err(|_| SignerError::InvalidPublicKey)?;
    let verifying_key =
        VerifyingKey::from_bytes(&public.0).map_err(|_| SignerError::InvalidPublicKey)?;
    let signature = Ed25519Signature::from_bytes(signature_bytes);
    verifying_key
        .verify_strict(transaction_hash, &signature)
        .map_err(|_| TransactionSigningError::InvalidSignature)
}

pub fn parse_transaction_envelope_xdr(
    xdr: &[u8],
) -> Result<TransactionEnvelope, TransactionSigningError> {
    TransactionEnvelope::from_xdr(xdr, Limits::none()).map_err(TransactionSigningError::Xdr)
}

pub fn transaction_envelope_xdr(
    envelope: &TransactionEnvelope,
) -> Result<Vec<u8>, TransactionSigningError> {
    envelope
        .to_xdr(Limits::none())
        .map_err(TransactionSigningError::Xdr)
}

#[derive(Debug, Error)]
pub enum TransactionSigningError {
    #[error("invalid Stellar transaction XDR")]
    Xdr(#[source] stellar_xdr::Error),
    #[error(transparent)]
    Signer(#[from] SignerError),
    #[error("signer returned a signature for the wrong key or transaction hash")]
    InvalidSignature,
    #[error("signer has already signed this transaction")]
    DuplicateSignature,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::{ExternalEd25519Signer, SoftwareSigner};

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const TESTNET: &str = "Test SDF Network ; September 2015";
    const TRANSACTION_HASH_HEX: &str =
        "dd8d4e2abf55d45c62805bfaae02baf1143f8c79b457dc0db6e1887902f9e43e";
    const SIGNATURE_HEX: &str = concat!(
        "99254edb377824d7162192be9a4afc95e1943598051022de0e64fb1e75c75b43",
        "6e7cf492d41a6f3445728b4afbf640ec3d472f22141b5d1fdf1520c0ed758d09"
    );
    const UNSIGNED_XDR_HEX: &str = concat!(
        "0000000200000000d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325",
        "af021a68f707511a000000640000000000000001000000000000000000000000",
        "0000000000000000"
    );
    const SIGNED_XDR_HEX: &str = concat!(
        "0000000200000000d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325",
        "af021a68f707511a000000640000000000000001000000000000000000000000",
        "0000000000000001f707511a0000004099254edb377824d7162192be9a4afc95",
        "e1943598051022de0e64fb1e75c75b436e7cf492d41a6f3445728b4afbf640e",
        "c3d472f22141b5d1fdf1520c0ed758d09"
    );

    fn decode_hex(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }

    fn decode_hex_array<const N: usize>(hex: &str) -> [u8; N] {
        decode_hex(hex).try_into().unwrap()
    }

    #[test]
    fn derives_stellar_testnet_network_id() {
        let expected: [u8; 32] = decode_hex(
            "cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472",
        )
        .try_into()
        .unwrap();

        assert_eq!(network_id(TESTNET), expected);
    }

    #[test]
    fn hashes_classic_transaction_with_official_xdr_semantics() {
        let envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let expected = decode_hex_array::<32>(TRANSACTION_HASH_HEX);

        assert_eq!(transaction_hash(&envelope, TESTNET).unwrap(), expected);
    }

    #[test]
    fn adds_expected_decorated_signature_to_classic_envelope() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();

        sign_transaction_envelope(&mut envelope, TESTNET, &signer).unwrap();

        assert_eq!(transaction_envelope_xdr(&envelope).unwrap(), decode_hex(SIGNED_XDR_HEX));
    }

    #[test]
    fn external_signer_receives_public_transaction_material() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let captured = Arc::new(Mutex::new(None));
        let provider_capture = Arc::clone(&captured);
        let signature = decode_hex_array::<64>(SIGNATURE_HEX);
        let signer = ExternalEd25519Signer::new(PUBLIC, move |request| {
            *provider_capture.lock().unwrap() = Some(request.clone());
            Ok(signature)
        })
        .unwrap();

        sign_transaction_envelope(&mut envelope, TESTNET, &signer).unwrap();

        let request = captured.lock().unwrap().clone().unwrap();
        assert_eq!(request.transaction_hash, decode_hex_array::<32>(TRANSACTION_HASH_HEX));
        assert_eq!(request.transaction_xdr, decode_hex(UNSIGNED_XDR_HEX));
        assert_eq!(request.network_passphrase, TESTNET);
        assert_eq!(transaction_envelope_xdr(&envelope).unwrap(), decode_hex(SIGNED_XDR_HEX));
    }

    #[test]
    fn rejects_invalid_external_signature_before_mutating_envelope() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let signer = ExternalEd25519Signer::new(PUBLIC, |_| Ok([0u8; 64])).unwrap();

        let result = sign_transaction_envelope(&mut envelope, TESTNET, &signer);

        assert!(matches!(result, Err(TransactionSigningError::InvalidSignature)));
        assert_eq!(transaction_envelope_xdr(&envelope).unwrap(), decode_hex(UNSIGNED_XDR_HEX));
    }

    #[test]
    fn external_provider_failure_does_not_mutate_envelope() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let signer = ExternalEd25519Signer::new(PUBLIC, |_| {
            Err(SignerError::ExternalProvider("device disconnected".to_owned()))
        })
        .unwrap();

        let result = sign_transaction_envelope(&mut envelope, TESTNET, &signer);

        assert!(matches!(
            result,
            Err(TransactionSigningError::Signer(SignerError::ExternalProvider(message)))
                if message == "device disconnected"
        ));
        assert_eq!(transaction_envelope_xdr(&envelope).unwrap(), decode_hex(UNSIGNED_XDR_HEX));
    }

    #[test]
    fn rejects_duplicate_decorated_signature() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();

        sign_transaction_envelope(&mut envelope, TESTNET, &signer).unwrap();
        assert!(matches!(
            sign_transaction_envelope(&mut envelope, TESTNET, &signer),
            Err(TransactionSigningError::DuplicateSignature)
        ));
    }
}

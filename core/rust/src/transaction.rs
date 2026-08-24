use sha2::{Digest, Sha256};
use stellar_xdr::{
    DecoratedSignature, Limits, ReadXdr, Signature, SignatureHint, TransactionEnvelope, WriteXdr,
};
use thiserror::Error;

use crate::signer::ClassicSigner;

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

pub fn sign_transaction_envelope<S: ClassicSigner>(
    envelope: &mut TransactionEnvelope,
    network_passphrase: &str,
    signer: &S,
) -> Result<(), TransactionSigningError> {
    let hash = transaction_hash(envelope, network_passphrase)?;
    let decorated = DecoratedSignature {
        hint: SignatureHint(signer.signature_hint()),
        signature: Signature(
            signer
                .sign_transaction_hash(&hash)
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
    #[error("signer has already signed this transaction")]
    DuplicateSignature,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SoftwareSigner;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const TESTNET: &str = "Test SDF Network ; September 2015";
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
        let expected: [u8; 32] = decode_hex(
            "dd8d4e2abf55d45c62805bfaae02baf1143f8c79b457dc0db6e1887902f9e43e",
        )
        .try_into()
        .unwrap();

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

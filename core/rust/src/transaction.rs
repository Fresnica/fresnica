use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::PublicKey;
use stellar_xdr::{
    DecoratedSignature, Limits, ReadXdr, Signature, SignatureHint, SignerKey, TransactionEnvelope,
    Uint256, WriteXdr,
};
use thiserror::Error;

use crate::signer::{ClassicSigner, SignerError, TransactionSigningRequest};

// Finite library-level decoder bound. Wallet/Application ingress may apply stricter policy.
const XDR_DECODE_MAX_DEPTH: u32 = 500;

fn xdr_decode_limits(encoded_len: usize) -> Limits {
    Limits {
        depth: XDR_DECODE_MAX_DEPTH,
        len: encoded_len,
    }
}

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

pub fn transaction_envelope_has_valid_signature(
    envelope: &TransactionEnvelope,
    network_passphrase: &str,
    signer_public_key: &str,
) -> Result<bool, TransactionSigningError> {
    let public =
        PublicKey::from_string(signer_public_key).map_err(|_| SignerError::InvalidPublicKey)?;
    transaction_envelope_satisfies_signer_key(
        envelope,
        network_passphrase,
        &SignerKey::Ed25519(Uint256(public.0)),
    )
}

pub fn transaction_envelope_satisfies_signer_key(
    envelope: &TransactionEnvelope,
    network_passphrase: &str,
    signer: &SignerKey,
) -> Result<bool, TransactionSigningError> {
    let signatures = match envelope {
        TransactionEnvelope::TxV0(value) => &value.signatures,
        TransactionEnvelope::Tx(value) => &value.signatures,
        TransactionEnvelope::TxFeeBump(value) => &value.signatures,
    };

    match signer {
        SignerKey::Ed25519(public) => {
            let verifying_key =
                VerifyingKey::from_bytes(&public.0).map_err(|_| SignerError::InvalidPublicKey)?;
            let hash = transaction_hash(envelope, network_passphrase)?;
            let hint = signature_hint(&public.0);
            Ok(signatures.iter().any(|decorated| {
                decorated.hint == hint
                    && <[u8; 64]>::try_from(decorated.signature.0.as_slice())
                        .map(|bytes| {
                            verifying_key
                                .verify_strict(&hash, &Ed25519Signature::from_bytes(&bytes))
                                .is_ok()
                        })
                        .unwrap_or(false)
            }))
        }
        SignerKey::PreAuthTx(expected) => {
            Ok(expected.0 == transaction_hash(envelope, network_passphrase)?)
        }
        SignerKey::HashX(expected) => {
            let hint = signature_hint(&expected.0);
            Ok(signatures.iter().any(|decorated| {
                decorated.hint == hint
                    && <[u8; 32]>::from(Sha256::digest(decorated.signature.0.as_slice()))
                        == expected.0
            }))
        }
        SignerKey::Ed25519SignedPayload(signed) => {
            let verifying_key = VerifyingKey::from_bytes(&signed.ed25519.0)
                .map_err(|_| SignerError::InvalidPublicKey)?;
            let payload = signed.payload.as_slice();
            let public_hint = signature_hint(&signed.ed25519.0);
            let payload_hint = signature_hint(payload);
            let hint = SignatureHint(core::array::from_fn(|index| {
                public_hint.0[index] ^ payload_hint.0[index]
            }));
            Ok(signatures.iter().any(|decorated| {
                decorated.hint == hint
                    && <[u8; 64]>::try_from(decorated.signature.0.as_slice())
                        .map(|bytes| {
                            verifying_key
                                .verify_strict(payload, &Ed25519Signature::from_bytes(&bytes))
                                .is_ok()
                        })
                        .unwrap_or(false)
            }))
        }
    }
}

fn signature_hint(bytes: &[u8]) -> SignatureHint {
    let mut hint = [0u8; 4];
    if bytes.len() < hint.len() {
        hint[..bytes.len()].copy_from_slice(bytes);
    } else {
        hint.copy_from_slice(&bytes[bytes.len() - 4..]);
    }
    SignatureHint(hint)
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
    *signatures = updated.try_into().map_err(TransactionSigningError::Xdr)?;
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
    TransactionEnvelope::from_xdr(xdr, xdr_decode_limits(xdr.len()))
        .map_err(TransactionSigningError::Xdr)
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

    use ed25519_dalek::{Signer as _, SigningKey};
    use stellar_strkey::ed25519::PrivateKey;
    use stellar_xdr::SignerKeyEd25519SignedPayload;

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

    fn set_signatures(envelope: &mut TransactionEnvelope, signatures: Vec<DecoratedSignature>) {
        let signatures = signatures.try_into().unwrap();
        match envelope {
            TransactionEnvelope::TxV0(value) => value.signatures = signatures,
            TransactionEnvelope::Tx(value) => value.signatures = signatures,
            TransactionEnvelope::TxFeeBump(value) => value.signatures = signatures,
        }
    }

    #[test]
    fn xdr_decode_limits_bound_length_and_recursive_depth() {
        use stellar_xdr::ScVal;

        let limits = xdr_decode_limits(1234);
        assert_eq!(limits.len, 1234);
        assert_eq!(limits.depth, XDR_DECODE_MAX_DEPTH);

        fn nested_scval_xdr(levels: usize) -> Vec<u8> {
            const SCV_VEC: [u8; 4] = 16i32.to_be_bytes();
            const PRESENT: [u8; 4] = 1u32.to_be_bytes();
            const ONE_ITEM: [u8; 4] = 1u32.to_be_bytes();
            const SCV_VOID: [u8; 4] = 1i32.to_be_bytes();

            let mut raw = Vec::with_capacity(levels * 12 + 4);
            for _ in 0..levels {
                raw.extend_from_slice(&SCV_VEC);
                raw.extend_from_slice(&PRESENT);
                raw.extend_from_slice(&ONE_ITEM);
            }
            raw.extend_from_slice(&SCV_VOID);
            raw
        }

        let accepted = nested_scval_xdr(8);
        assert!(ScVal::from_xdr(&accepted, xdr_decode_limits(accepted.len())).is_ok());

        let rejected = nested_scval_xdr(1_000);
        assert!(ScVal::from_xdr(&rejected, xdr_decode_limits(rejected.len())).is_err());
    }

    #[test]
    fn derives_stellar_testnet_network_id() {
        let expected: [u8; 32] =
            decode_hex("cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472")
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

        assert_eq!(
            transaction_envelope_xdr(&envelope).unwrap(),
            decode_hex(SIGNED_XDR_HEX)
        );
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
        assert_eq!(
            request.transaction_hash,
            decode_hex_array::<32>(TRANSACTION_HASH_HEX)
        );
        assert_eq!(request.transaction_xdr, decode_hex(UNSIGNED_XDR_HEX));
        assert_eq!(request.network_passphrase, TESTNET);
        assert_eq!(
            transaction_envelope_xdr(&envelope).unwrap(),
            decode_hex(SIGNED_XDR_HEX)
        );
    }

    #[test]
    fn rejects_invalid_external_signature_before_mutating_envelope() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let signer = ExternalEd25519Signer::new(PUBLIC, |_| Ok([0u8; 64])).unwrap();

        let result = sign_transaction_envelope(&mut envelope, TESTNET, &signer);

        assert!(matches!(
            result,
            Err(TransactionSigningError::InvalidSignature)
        ));
        assert_eq!(
            transaction_envelope_xdr(&envelope).unwrap(),
            decode_hex(UNSIGNED_XDR_HEX)
        );
    }

    #[test]
    fn external_provider_failure_does_not_mutate_envelope() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let signer = ExternalEd25519Signer::new(PUBLIC, |_| {
            Err(SignerError::ExternalProvider(
                "device disconnected".to_owned(),
            ))
        })
        .unwrap();

        let result = sign_transaction_envelope(&mut envelope, TESTNET, &signer);

        assert!(matches!(
            result,
            Err(TransactionSigningError::Signer(SignerError::ExternalProvider(message)))
                if message == "device disconnected"
        ));
        assert_eq!(
            transaction_envelope_xdr(&envelope).unwrap(),
            decode_hex(UNSIGNED_XDR_HEX)
        );
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

    #[test]
    fn detects_valid_existing_signature_for_expected_key_and_network() {
        let envelope = parse_transaction_envelope_xdr(&decode_hex(SIGNED_XDR_HEX)).unwrap();

        assert!(transaction_envelope_has_valid_signature(&envelope, TESTNET, PUBLIC).unwrap());
        assert!(!transaction_envelope_has_valid_signature(
            &envelope,
            "Public Global Stellar Network ; September 2015",
            PUBLIC,
        )
        .unwrap());
    }

    #[test]
    fn typed_signer_verifier_recognizes_preauth() {
        let envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let signer = SignerKey::PreAuthTx(Uint256(transaction_hash(&envelope, TESTNET).unwrap()));

        assert!(transaction_envelope_satisfies_signer_key(&envelope, TESTNET, &signer).unwrap());
    }

    #[test]
    fn typed_signer_verifier_recognizes_hash_x() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let preimage = b"fresnica-hash-x";
        let hash: [u8; 32] = Sha256::digest(preimage).into();
        set_signatures(
            &mut envelope,
            vec![DecoratedSignature {
                hint: signature_hint(&hash),
                signature: Signature(preimage.to_vec().try_into().unwrap()),
            }],
        );

        assert!(transaction_envelope_satisfies_signer_key(
            &envelope,
            TESTNET,
            &SignerKey::HashX(Uint256(hash)),
        )
        .unwrap());
    }

    #[test]
    fn typed_signer_verifier_recognizes_signed_payload() {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let private = PrivateKey::from_string(SECRET).unwrap();
        let signing_key = SigningKey::from_bytes(&private.0);
        let payload = b"x";
        let public = signing_key.verifying_key().to_bytes();
        let public_hint = signature_hint(&public);
        let payload_hint = signature_hint(payload);
        let hint = SignatureHint(core::array::from_fn(|index| {
            public_hint.0[index] ^ payload_hint.0[index]
        }));
        set_signatures(
            &mut envelope,
            vec![DecoratedSignature {
                hint,
                signature: Signature(
                    signing_key
                        .sign(payload)
                        .to_bytes()
                        .to_vec()
                        .try_into()
                        .unwrap(),
                ),
            }],
        );
        let signer = SignerKey::Ed25519SignedPayload(SignerKeyEd25519SignedPayload {
            ed25519: Uint256(public),
            payload: payload.to_vec().try_into().unwrap(),
        });

        assert!(transaction_envelope_satisfies_signer_key(&envelope, TESTNET, &signer).unwrap());
    }

    #[test]
    fn unsigned_envelope_has_no_valid_signature() {
        let envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();

        assert!(!transaction_envelope_has_valid_signature(&envelope, TESTNET, PUBLIC).unwrap());
    }
}

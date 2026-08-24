use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_core::{
    parse_transaction_envelope_xdr, sign_transaction_envelope, transaction_envelope_xdr,
    transaction_hash, ClassicSigner, SoftwareSigner, TransactionSigningRequest,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TransactionSigningVectors {
    schema: String,
    cases: Vec<TransactionSigningVector>,
}

#[derive(Debug, Deserialize)]
struct TransactionSigningVector {
    name: String,
    network_passphrase: String,
    secret: String,
    public_key: String,
    unsigned_xdr_base64: String,
    transaction_hash_hex: String,
    signature_hex: String,
    signature_hint_hex: String,
    signed_xdr_base64: String,
}

fn decode_hex_array<const N: usize>(hex: &str) -> [u8; N] {
    assert_eq!(hex.len(), N * 2);
    let mut out = [0u8; N];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
    }
    out
}

#[test]
fn classic_transaction_signing_matches_cross_language_vectors() {
    let raw = include_str!("../../../spec/test-vectors/transaction-signing-v1.json");
    let vectors: TransactionSigningVectors = serde_json::from_str(raw).unwrap();

    assert_eq!(vectors.schema, "fresnica-transaction-signing-v1");
    assert!(!vectors.cases.is_empty());

    for vector in vectors.cases {
        let unsigned_xdr = STANDARD.decode(&vector.unsigned_xdr_base64).unwrap();
        let expected_hash = decode_hex_array::<32>(&vector.transaction_hash_hex);
        let expected_signature = decode_hex_array::<64>(&vector.signature_hex);
        let expected_hint = decode_hex_array::<4>(&vector.signature_hint_hex);
        let signer = SoftwareSigner::from_secret(&vector.secret)
            .unwrap_or_else(|error| panic!("{} signer failed: {error}", vector.name));
        let mut envelope = parse_transaction_envelope_xdr(&unsigned_xdr)
            .unwrap_or_else(|error| panic!("{} XDR parse failed: {error}", vector.name));

        assert_eq!(signer.public_key(), vector.public_key, "{}", vector.name);
        assert_eq!(
            transaction_hash(&envelope, &vector.network_passphrase).unwrap(),
            expected_hash,
            "{}",
            vector.name
        );
        assert_eq!(signer.signature_hint().unwrap(), expected_hint, "{}", vector.name);
        assert_eq!(
            signer
                .sign_transaction(&TransactionSigningRequest {
                    transaction_hash: expected_hash,
                    transaction_xdr: unsigned_xdr.clone(),
                    network_passphrase: vector.network_passphrase.clone(),
                })
                .unwrap(),
            expected_signature,
            "{}",
            vector.name
        );

        sign_transaction_envelope(&mut envelope, &vector.network_passphrase, &signer)
            .unwrap_or_else(|error| panic!("{} signing failed: {error}", vector.name));
        assert_eq!(
            STANDARD.encode(transaction_envelope_xdr(&envelope).unwrap()),
            vector.signed_xdr_base64,
            "{}",
            vector.name
        );
    }
}

use fresnica_core::{
    prepare_message_signing, sep53_message_hash, sep53_message_payload, sign_message,
    verify_message_signature, SoftwareSigner,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Vectors {
    schema: String,
    prefix_utf8: String,
    cases: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
struct Vector {
    name: String,
    message_hex: String,
    secret: String,
    public_key: String,
    encoded_message_hex: String,
    message_hash_hex: String,
    signature_hex: String,
}

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn sep53_message_signing_matches_cross_language_vectors() {
    let raw = include_str!("../../../spec/test-vectors/message-signing-v1.json");
    let vectors: Vectors = serde_json::from_str(raw).unwrap();

    assert_eq!(vectors.schema, "fresnica-message-signing-v1");
    assert_eq!(vectors.prefix_utf8, "Stellar Signed Message:\n");

    for vector in vectors.cases {
        let message = decode_hex(&vector.message_hex);
        let expected_payload = decode_hex(&vector.encoded_message_hex);
        let expected_hash: [u8; 32] = decode_hex(&vector.message_hash_hex).try_into().unwrap();
        let expected_signature: [u8; 64] = decode_hex(&vector.signature_hex).try_into().unwrap();

        assert_eq!(
            sep53_message_payload(&message),
            expected_payload,
            "{}",
            vector.name
        );
        assert_eq!(
            sep53_message_hash(&message),
            expected_hash,
            "{}",
            vector.name
        );

        let request = prepare_message_signing(&message);
        assert_eq!(request.message, message, "{}", vector.name);
        assert_eq!(request.encoded_message, expected_payload, "{}", vector.name);
        assert_eq!(request.message_hash, expected_hash, "{}", vector.name);

        let signer = SoftwareSigner::from_secret(&vector.secret).unwrap();
        let signature = sign_message(&request.message, &signer).unwrap();
        assert_eq!(signature, expected_signature, "{}", vector.name);
        verify_message_signature(&vector.public_key, &request.message, &signature).unwrap();
    }
}

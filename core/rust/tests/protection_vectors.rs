use fresnica_core::{
    decrypt_secret, decrypt_secret_with_key, KeySecretEnvelope, PasswordSecretEnvelope,
    SecretStoreError,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct ProtectionVectors {
    schema: String,
    payload: Value,
    password: PasswordVector,
    system: SystemVector,
}

#[derive(Debug, Deserialize)]
struct PasswordVector {
    password: String,
    envelope: PasswordSecretEnvelope,
}

#[derive(Debug, Deserialize)]
struct SystemVector {
    key_hex: String,
    envelope: KeySecretEnvelope,
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
fn protection_envelopes_match_cross_language_vectors() {
    let raw = include_str!("../../../spec/test-vectors/protection-v1.json");
    let vectors: ProtectionVectors = serde_json::from_str(raw).unwrap();

    assert_eq!(vectors.schema, "fresnica-protection-v1");
    assert_eq!(
        decrypt_secret(&vectors.password.envelope, &vectors.password.password).unwrap(),
        vectors.payload
    );
    assert_eq!(
        decrypt_secret(&vectors.password.envelope, "wrong").unwrap_err(),
        SecretStoreError::InvalidPassword
    );

    let key = decode_hex_array::<32>(&vectors.system.key_hex);
    assert_eq!(
        decrypt_secret_with_key(&vectors.system.envelope, &key).unwrap(),
        vectors.payload
    );
}

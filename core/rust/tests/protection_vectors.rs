use fresnica_core::{
    decrypt_secret, decrypt_secret_with_unlock_key, derive_unlock_key, PasswordSecretEnvelope,
    SecretStoreError,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct ProtectionVectors {
    schema: String,
    payload: Value,
    password: PasswordVector,
}

#[derive(Debug, Deserialize)]
struct PasswordVector {
    password: String,
    envelope: PasswordSecretEnvelope,
}

#[test]
fn protection_envelope_matches_cross_language_vector_and_unlock_key_path() {
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

    let unlock_key = derive_unlock_key(&vectors.password.envelope, &vectors.password.password)
        .unwrap();
    assert_eq!(
        decrypt_secret_with_unlock_key(&vectors.password.envelope, &unlock_key).unwrap(),
        vectors.payload
    );
}

use aes_gcm::{
    aead::{Aead, KeyInit, Nonce, Payload},
    Aes256Gcm,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use scrypt::{scrypt, Params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use zeroize::Zeroizing;

const AAD: &[u8] = b"fresnica-wallet-secret-v1";
const KEY_AAD: &[u8] = b"fresnica-wallet-secret-key-v1";
const SCRYPT_LOG_N: u8 = 15;
pub const SCRYPT_N: u64 = 1 << SCRYPT_LOG_N;
pub const SCRYPT_R: u32 = 8;
pub const SCRYPT_P: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScryptEnvelope {
    pub name: String,
    pub n: u64,
    pub r: u32,
    pub p: u32,
    pub salt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PasswordSecretEnvelope {
    pub version: u8,
    pub cipher: String,
    pub kdf: ScryptEnvelope,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeySecretEnvelope {
    pub version: u8,
    pub cipher: String,
    pub nonce: String,
    pub ciphertext: String,
}

pub fn encrypt_secret(
    payload: &Value,
    password: &str,
) -> Result<PasswordSecretEnvelope, SecretStoreError> {
    if password.is_empty() {
        return Err(SecretStoreError::EmptyPassword);
    }

    let salt = random_array::<16>()?;
    let nonce = random_array::<12>()?;
    encrypt_secret_with_material(payload, password, &salt, &nonce)
}

pub fn decrypt_secret(
    envelope: &PasswordSecretEnvelope,
    password: &str,
) -> Result<Value, SecretStoreError> {
    validate_password_envelope(envelope)?;
    if password.is_empty() {
        return Err(SecretStoreError::EmptyPassword);
    }

    let salt = decode_array::<16>(&envelope.kdf.salt)?;
    let nonce = decode_array::<12>(&envelope.nonce)?;
    let ciphertext = decode_base64(&envelope.ciphertext)?;
    let key = derive_key(password, &salt)?;
    let plaintext = decrypt_aead(&key, &nonce, AAD, &ciphertext)
        .map_err(|_| SecretStoreError::InvalidPassword)?;
    decode_payload(plaintext)
}

pub fn encrypt_secret_with_key(
    payload: &Value,
    key: &[u8; 32],
) -> Result<KeySecretEnvelope, SecretStoreError> {
    let nonce = random_array::<12>()?;
    encrypt_secret_with_key_and_nonce(payload, key, &nonce)
}

pub fn decrypt_secret_with_key(
    envelope: &KeySecretEnvelope,
    key: &[u8; 32],
) -> Result<Value, SecretStoreError> {
    validate_key_envelope(envelope)?;
    let nonce = decode_array::<12>(&envelope.nonce)?;
    let ciphertext = decode_base64(&envelope.ciphertext)?;
    let plaintext = decrypt_aead(key, &nonce, KEY_AAD, &ciphertext)
        .map_err(|_| SecretStoreError::AuthenticationFailed)?;
    decode_payload(plaintext)
}

fn encrypt_secret_with_material(
    payload: &Value,
    password: &str,
    salt: &[u8; 16],
    nonce: &[u8; 12],
) -> Result<PasswordSecretEnvelope, SecretStoreError> {
    if password.is_empty() {
        return Err(SecretStoreError::EmptyPassword);
    }

    let key = derive_key(password, salt)?;
    let plaintext = encode_payload(payload)?;
    let ciphertext = encrypt_aead(&key, nonce, AAD, &plaintext)?;

    Ok(PasswordSecretEnvelope {
        version: 1,
        cipher: "aes-256-gcm".to_owned(),
        kdf: ScryptEnvelope {
            name: "scrypt".to_owned(),
            n: SCRYPT_N,
            r: SCRYPT_R,
            p: SCRYPT_P,
            salt: STANDARD.encode(salt),
        },
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
}

fn encrypt_secret_with_key_and_nonce(
    payload: &Value,
    key: &[u8; 32],
    nonce: &[u8; 12],
) -> Result<KeySecretEnvelope, SecretStoreError> {
    let plaintext = encode_payload(payload)?;
    let ciphertext = encrypt_aead(key, nonce, KEY_AAD, &plaintext)?;

    Ok(KeySecretEnvelope {
        version: 1,
        cipher: "aes-256-gcm".to_owned(),
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
}

fn validate_password_envelope(
    envelope: &PasswordSecretEnvelope,
) -> Result<(), SecretStoreError> {
    if envelope.version != 1 || envelope.cipher != "aes-256-gcm" {
        return Err(SecretStoreError::UnsupportedEncryptionFormat);
    }
    if envelope.kdf.name != "scrypt" {
        return Err(SecretStoreError::UnsupportedKdfFormat);
    }
    if (envelope.kdf.n, envelope.kdf.r, envelope.kdf.p)
        != (SCRYPT_N, SCRYPT_R, SCRYPT_P)
    {
        return Err(SecretStoreError::UnsupportedKdfParameters);
    }
    Ok(())
}

fn validate_key_envelope(envelope: &KeySecretEnvelope) -> Result<(), SecretStoreError> {
    if envelope.version != 1 || envelope.cipher != "aes-256-gcm" {
        return Err(SecretStoreError::UnsupportedEncryptionFormat);
    }
    Ok(())
}

fn derive_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, SecretStoreError> {
    let params = Params::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P)
        .map_err(|_| SecretStoreError::CryptoFailure)?;
    let mut key = Zeroizing::new([0u8; 32]);
    scrypt(password.as_bytes(), salt, &params, &mut key[..])
        .map_err(|_| SecretStoreError::CryptoFailure)?;
    Ok(key)
}

fn encode_payload(payload: &Value) -> Result<Zeroizing<Vec<u8>>, SecretStoreError> {
    serde_json::to_vec(payload)
        .map(Zeroizing::new)
        .map_err(|_| SecretStoreError::Corrupted)
}

fn decode_payload(plaintext: Zeroizing<Vec<u8>>) -> Result<Value, SecretStoreError> {
    serde_json::from_slice(&plaintext).map_err(|_| SecretStoreError::Corrupted)
}

fn encrypt_aead(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, SecretStoreError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| SecretStoreError::CryptoFailure)?;
    let nonce = Nonce::<Aes256Gcm>::try_from(&nonce[..])
        .map_err(|_| SecretStoreError::CryptoFailure)?;
    cipher
        .encrypt(&nonce, Payload { msg: plaintext, aad })
        .map_err(|_| SecretStoreError::CryptoFailure)
}

fn decrypt_aead(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, SecretStoreError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| SecretStoreError::CryptoFailure)?;
    let nonce = Nonce::<Aes256Gcm>::try_from(&nonce[..])
        .map_err(|_| SecretStoreError::Corrupted)?;
    cipher
        .decrypt(&nonce, Payload { msg: ciphertext, aad })
        .map(Zeroizing::new)
        .map_err(|_| SecretStoreError::AuthenticationFailed)
}

fn decode_base64(value: &str) -> Result<Vec<u8>, SecretStoreError> {
    STANDARD
        .decode(value)
        .map_err(|_| SecretStoreError::Corrupted)
}

fn decode_array<const N: usize>(value: &str) -> Result<[u8; N], SecretStoreError> {
    decode_base64(value)?
        .try_into()
        .map_err(|_| SecretStoreError::Corrupted)
}

fn random_array<const N: usize>() -> Result<[u8; N], SecretStoreError> {
    let mut value = [0u8; N];
    getrandom::fill(&mut value).map_err(|_| SecretStoreError::RandomUnavailable)?;
    Ok(value)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretStoreError {
    #[error("wallet password cannot be empty")]
    EmptyPassword,
    #[error("invalid wallet password")]
    InvalidPassword,
    #[error("unsupported wallet encryption format")]
    UnsupportedEncryptionFormat,
    #[error("unsupported wallet key derivation format")]
    UnsupportedKdfFormat,
    #[error("unsupported wallet KDF parameters")]
    UnsupportedKdfParameters,
    #[error("wallet secret data is corrupted")]
    Corrupted,
    #[error("protected wallet secret failed authentication")]
    AuthenticationFailed,
    #[error("secure randomness is unavailable")]
    RandomUnavailable,
    #[error("wallet secret cryptography failed")]
    CryptoFailure,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize)]
    struct ProtectionVectors {
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

    fn shared_vectors() -> ProtectionVectors {
        serde_json::from_str(include_str!("../../../spec/test-vectors/protection-v1.json")).unwrap()
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
    fn password_roundtrip_rejects_wrong_password() {
        let payload = json!({"kind": "secret", "secret": "S..."});
        let envelope = encrypt_secret(&payload, "correct").unwrap();

        assert_eq!(decrypt_secret(&envelope, "correct").unwrap(), payload);
        assert_eq!(
            decrypt_secret(&envelope, "wrong").unwrap_err(),
            SecretStoreError::InvalidPassword
        );
    }

    #[test]
    fn key_roundtrip_authenticates_ciphertext() {
        let payload = json!({"kind": "secret", "secret": "S..."});
        let key = [7u8; 32];
        let envelope = encrypt_secret_with_key(&payload, &key).unwrap();

        assert_eq!(decrypt_secret_with_key(&envelope, &key).unwrap(), payload);
        assert_eq!(
            decrypt_secret_with_key(&envelope, &[8u8; 32]).unwrap_err(),
            SecretStoreError::AuthenticationFailed
        );
    }

    #[test]
    fn password_encryption_matches_shared_python_vector_byte_for_byte() {
        let vectors = shared_vectors();
        let salt = decode_array::<16>(&vectors.password.envelope.kdf.salt).unwrap();
        let nonce = decode_array::<12>(&vectors.password.envelope.nonce).unwrap();

        assert_eq!(
            encrypt_secret_with_material(
                &vectors.payload,
                &vectors.password.password,
                &salt,
                &nonce,
            )
            .unwrap(),
            vectors.password.envelope
        );
    }

    #[test]
    fn system_key_encryption_matches_shared_python_vector_byte_for_byte() {
        let vectors = shared_vectors();
        let key = decode_hex_array::<32>(&vectors.system.key_hex);
        let nonce = decode_array::<12>(&vectors.system.envelope.nonce).unwrap();

        assert_eq!(
            encrypt_secret_with_key_and_nonce(&vectors.payload, &key, &nonce).unwrap(),
            vectors.system.envelope
        );
    }
}

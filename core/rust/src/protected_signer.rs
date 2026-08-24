use serde_json::{Map, Value};
use stellar_strkey::ed25519::PublicKey;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    derive_classic_signer, detect_mnemonic_language, ClassicSigner, ProtectionCredential,
    ProtectionError, ProtectionRegistry, SignerError, SoftwareSigner, WalletDerivationError,
};

pub fn unlock_software_signer(
    registry: &ProtectionRegistry,
    envelope: &Value,
    credential: &ProtectionCredential,
    expected_public_key: &str,
) -> Result<SoftwareSigner, ProtectedSignerError> {
    let payload = registry.unprotect(envelope, credential)?;
    let signer = software_signer_from_payload(payload)?;
    let expected = PublicKey::from_string(expected_public_key.trim())
        .map_err(|_| ProtectedSignerError::InvalidExpectedPublicKey)?;

    if signer.public_key() != format!("{expected}") {
        return Err(ProtectedSignerError::IdentityMismatch);
    }

    Ok(signer)
}

fn software_signer_from_payload(payload: Value) -> Result<SoftwareSigner, ProtectedSignerError> {
    let mut object = match payload {
        Value::Object(object) => object,
        _ => return Err(ProtectedSignerError::InvalidSigningMaterial),
    };
    let kind = take_string(&mut object, "kind")?;

    match kind.as_str() {
        "secret" => signer_from_secret_payload(&mut object),
        "mnemonic" => signer_from_mnemonic_payload(&mut object),
        _ => Err(ProtectedSignerError::UnsupportedSigningMaterial),
    }
}

fn signer_from_secret_payload(
    object: &mut Map<String, Value>,
) -> Result<SoftwareSigner, ProtectedSignerError> {
    let secret = Zeroizing::new(take_string(object, "secret")?);
    SoftwareSigner::from_secret(&secret).map_err(Into::into)
}

fn signer_from_mnemonic_payload(
    object: &mut Map<String, Value>,
) -> Result<SoftwareSigner, ProtectedSignerError> {
    let mnemonic = Zeroizing::new(take_string(object, "mnemonic")?);
    let passphrase = Zeroizing::new(take_optional_string(object, "mnemonic_passphrase")?);
    let index = take_optional_index(object, "index")?;
    let language = take_optional_language(object, "language", &mnemonic)?;

    derive_classic_signer(&mnemonic, &passphrase, index, &language).map_err(Into::into)
}

fn take_string(
    object: &mut Map<String, Value>,
    key: &str,
) -> Result<String, ProtectedSignerError> {
    match object.remove(key) {
        Some(Value::String(value)) => Ok(value),
        _ => Err(ProtectedSignerError::InvalidSigningMaterial),
    }
}

fn take_optional_string(
    object: &mut Map<String, Value>,
    key: &str,
) -> Result<String, ProtectedSignerError> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value),
        _ => Err(ProtectedSignerError::InvalidSigningMaterial),
    }
}

fn take_optional_index(
    object: &mut Map<String, Value>,
    key: &str,
) -> Result<usize, ProtectedSignerError> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(0),
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(ProtectedSignerError::InvalidSigningMaterial),
        _ => Err(ProtectedSignerError::InvalidSigningMaterial),
    }
}

fn take_optional_language(
    object: &mut Map<String, Value>,
    key: &str,
    mnemonic: &str,
) -> Result<String, ProtectedSignerError> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(detect_mnemonic_language(mnemonic)?.to_owned()),
        Some(Value::String(value)) if !value.is_empty() => Ok(value),
        _ => Err(ProtectedSignerError::InvalidSigningMaterial),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtectedSignerError {
    #[error(transparent)]
    Protection(#[from] ProtectionError),
    #[error(transparent)]
    Signer(#[from] SignerError),
    #[error(transparent)]
    Derivation(#[from] WalletDerivationError),
    #[error("protected wallet signing material is invalid")]
    InvalidSigningMaterial,
    #[error("protected wallet signing material type is unsupported")]
    UnsupportedSigningMaterial,
    #[error("expected wallet public key is invalid")]
    InvalidExpectedPublicKey,
    #[error("decrypted wallet identity does not match metadata")]
    IdentityMismatch,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const CHINESE_MNEMONIC: &str = "这 的 的 的 的 的 的 的 的 的 的 人";
    const CHINESE_PUBLIC: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";

    #[test]
    fn unlocks_protected_secret_and_checks_identity() {
        let registry = ProtectionRegistry::new();
        let credential = ProtectionCredential::password("correct");
        let envelope = registry
            .protect(
                &json!({"kind": "secret", "secret": SECRET}),
                &credential,
            )
            .unwrap();

        let signer = unlock_software_signer(&registry, &envelope, &credential, PUBLIC).unwrap();

        assert_eq!(signer.public_key(), PUBLIC);
    }

    #[test]
    fn mnemonic_payload_detects_language_when_legacy_field_is_missing() {
        let signer = software_signer_from_payload(json!({
            "kind": "mnemonic",
            "mnemonic": CHINESE_MNEMONIC,
            "mnemonic_passphrase": "",
            "index": 0
        }))
        .unwrap();

        assert_eq!(signer.public_key(), CHINESE_PUBLIC);
    }

    #[test]
    fn rejects_decrypted_identity_that_does_not_match_metadata() {
        let registry = ProtectionRegistry::new();
        let credential = ProtectionCredential::password("correct");
        let envelope = registry
            .protect(
                &json!({"kind": "secret", "secret": SECRET}),
                &credential,
            )
            .unwrap();

        assert_eq!(
            unlock_software_signer(
                &registry,
                &envelope,
                &credential,
                "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY",
            )
            .unwrap_err(),
            ProtectedSignerError::IdentityMismatch
        );
    }

    #[test]
    fn rejects_unknown_signing_material_kind() {
        assert_eq!(
            software_signer_from_payload(json!({"kind": "future"})).unwrap_err(),
            ProtectedSignerError::UnsupportedSigningMaterial
        );
    }
}

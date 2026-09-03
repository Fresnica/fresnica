use std::fmt;

use serde_json::{Map, Value};
use stellar_strkey::ed25519::PublicKey;
use stellar_xdr::{SorobanAuthorizationEntry, TransactionEnvelope};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    derive_classic_signer, detect_mnemonic_language, sign_message,
    sign_soroban_authorization_entry, sign_transaction_envelope, MessageSigningError,
    ProtectionCredential, ProtectionError, ProtectionRegistry, SecretStoreError, SignerError,
    SoftwareSigner, SorobanAuthorizationSigningError, TransactionSigningError,
    WalletDerivationError, WalletUnlockKey,
};

pub enum ExportedSigningMaterial {
    Secret {
        secret: Zeroizing<String>,
    },
    Mnemonic {
        mnemonic: Zeroizing<String>,
        mnemonic_passphrase: Zeroizing<String>,
        index: usize,
        language: String,
    },
}

impl fmt::Debug for ExportedSigningMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Secret { .. } => formatter
                .debug_struct("Secret")
                .field("secret", &"<redacted>")
                .finish(),
            Self::Mnemonic {
                index, language, ..
            } => formatter
                .debug_struct("Mnemonic")
                .field("mnemonic", &"<redacted>")
                .field("mnemonic_passphrase", &"<redacted>")
                .field("index", index)
                .field("language", language)
                .finish(),
        }
    }
}

impl ExportedSigningMaterial {
    fn signer(&self) -> Result<SoftwareSigner, ProtectedSignerError> {
        match self {
            Self::Secret { secret } => SoftwareSigner::from_secret(secret).map_err(Into::into),
            Self::Mnemonic {
                mnemonic,
                mnemonic_passphrase,
                index,
                language,
            } => derive_classic_signer(mnemonic, mnemonic_passphrase, *index, language)
                .map_err(Into::into),
        }
    }
}

pub fn derive_verified_unlock_key(
    registry: &ProtectionRegistry,
    envelope: &Value,
    passcode: &str,
    expected_public_key: &str,
) -> Result<WalletUnlockKey, ProtectedSignerError> {
    let unlock_key = registry.derive_unlock_key(envelope, passcode)?;
    let payload = registry
        .unprotect_with_unlock_key(envelope, &unlock_key)
        .map_err(map_passcode_verification_error)?;
    let signer = software_signer_from_payload(payload)?;
    ensure_expected_identity(&signer, expected_public_key)?;
    drop(signer);
    Ok(unlock_key)
}

fn map_passcode_verification_error(error: ProtectionError) -> ProtectedSignerError {
    match error {
        ProtectionError::SecretStore(SecretStoreError::InvalidUnlockKey) => {
            ProtectedSignerError::Protection(ProtectionError::SecretStore(
                SecretStoreError::InvalidPassword,
            ))
        }
        other => ProtectedSignerError::Protection(other),
    }
}

pub fn unlock_software_signer(
    registry: &ProtectionRegistry,
    envelope: &Value,
    unlock_key: &WalletUnlockKey,
    expected_public_key: &str,
) -> Result<SoftwareSigner, ProtectedSignerError> {
    let payload = registry.unprotect_with_unlock_key(envelope, unlock_key)?;
    let signer = software_signer_from_payload(payload)?;
    ensure_expected_identity(&signer, expected_public_key)?;
    Ok(signer)
}

pub fn sign_protected_transaction_envelope(
    registry: &ProtectionRegistry,
    protected_envelope: &Value,
    unlock_key: &WalletUnlockKey,
    expected_public_key: &str,
    transaction_envelope: &mut TransactionEnvelope,
    network_passphrase: &str,
) -> Result<(), ProtectedSigningError> {
    let signer = unlock_software_signer(
        registry,
        protected_envelope,
        unlock_key,
        expected_public_key,
    )?;
    sign_transaction_envelope(transaction_envelope, network_passphrase, &signer)?;
    Ok(())
}

pub fn sign_protected_message(
    registry: &ProtectionRegistry,
    protected_envelope: &Value,
    unlock_key: &WalletUnlockKey,
    expected_public_key: &str,
    message: &[u8],
) -> Result<[u8; 64], ProtectedSigningError> {
    let signer = unlock_software_signer(
        registry,
        protected_envelope,
        unlock_key,
        expected_public_key,
    )?;
    Ok(sign_message(message, &signer)?)
}

pub fn sign_protected_soroban_authorization_entry(
    registry: &ProtectionRegistry,
    protected_envelope: &Value,
    unlock_key: &WalletUnlockKey,
    expected_public_key: &str,
    authorization_entry: &mut SorobanAuthorizationEntry,
    network_passphrase: &str,
) -> Result<(), ProtectedSigningError> {
    let signer = unlock_software_signer(
        registry,
        protected_envelope,
        unlock_key,
        expected_public_key,
    )?;
    sign_soroban_authorization_entry(authorization_entry, network_passphrase, &signer)?;
    Ok(())
}

pub fn export_signing_material(
    registry: &ProtectionRegistry,
    envelope: &Value,
    passcode: &str,
    expected_public_key: &str,
) -> Result<ExportedSigningMaterial, ProtectedSignerError> {
    let credential = ProtectionCredential::password(passcode);
    let payload = registry.unprotect(envelope, &credential)?;
    let material = signing_material_from_payload(payload)?;
    let signer = material.signer()?;
    ensure_expected_identity(&signer, expected_public_key)?;
    drop(signer);
    Ok(material)
}

fn software_signer_from_payload(payload: Value) -> Result<SoftwareSigner, ProtectedSignerError> {
    signing_material_from_payload(payload)?.signer()
}

fn signing_material_from_payload(
    payload: Value,
) -> Result<ExportedSigningMaterial, ProtectedSignerError> {
    let mut object = match payload {
        Value::Object(object) => object,
        _ => return Err(ProtectedSignerError::InvalidSigningMaterial),
    };

    let secret = take_optional_sensitive_string(&mut object, "secret");
    let mnemonic = take_optional_sensitive_string(&mut object, "mnemonic");
    let passphrase = take_optional_sensitive_string(&mut object, "mnemonic_passphrase");
    let secret = secret?;
    let mnemonic = mnemonic?;
    let passphrase = passphrase?;
    let kind = take_string(&mut object, "kind")?;

    match kind.as_str() {
        "secret" => Ok(ExportedSigningMaterial::Secret {
            secret: secret.ok_or(ProtectedSignerError::InvalidSigningMaterial)?,
        }),
        "mnemonic" => {
            let mnemonic = mnemonic.ok_or(ProtectedSignerError::InvalidSigningMaterial)?;
            let mnemonic_passphrase = passphrase.unwrap_or_else(|| Zeroizing::new(String::new()));
            let index = take_optional_index(&mut object, "index")?;
            let language = take_optional_language(&mut object, "language", &mnemonic)?;
            Ok(ExportedSigningMaterial::Mnemonic {
                mnemonic,
                mnemonic_passphrase,
                index,
                language,
            })
        }
        _ => Err(ProtectedSignerError::UnsupportedSigningMaterial),
    }
}

fn ensure_expected_identity(
    signer: &SoftwareSigner,
    expected_public_key: &str,
) -> Result<(), ProtectedSignerError> {
    let expected = PublicKey::from_string(expected_public_key.trim())
        .map_err(|_| ProtectedSignerError::InvalidExpectedPublicKey)?;

    if signer.public_key() != format!("{expected}") {
        return Err(ProtectedSignerError::IdentityMismatch);
    }
    Ok(())
}

fn take_string(object: &mut Map<String, Value>, key: &str) -> Result<String, ProtectedSignerError> {
    match object.remove(key) {
        Some(Value::String(value)) => Ok(value),
        _ => Err(ProtectedSignerError::InvalidSigningMaterial),
    }
}

fn take_optional_sensitive_string(
    object: &mut Map<String, Value>,
    key: &str,
) -> Result<Option<Zeroizing<String>>, ProtectedSignerError> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(Zeroizing::new(value))),
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

#[derive(Debug, Error)]
pub enum ProtectedSigningError {
    #[error(transparent)]
    Unlock(#[from] ProtectedSignerError),
    #[error(transparent)]
    Transaction(#[from] TransactionSigningError),
    #[error(transparent)]
    Message(#[from] MessageSigningError),
    #[error(transparent)]
    SorobanAuthorization(#[from] SorobanAuthorizationSigningError),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const CHINESE_MNEMONIC: &str = "这 的 的 的 的 的 的 的 的 的 的 人";
    const CHINESE_PUBLIC: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";
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

    fn protected_secret(registry: &ProtectionRegistry, passcode: &str) -> Value {
        registry
            .protect(
                &json!({"kind": "secret", "secret": SECRET}),
                &ProtectionCredential::password(passcode),
            )
            .unwrap()
    }

    #[test]
    fn derives_verified_unlock_key_and_unlocks_signer() {
        let registry = ProtectionRegistry::new();
        let envelope = protected_secret(&registry, "correct");
        let key = derive_verified_unlock_key(&registry, &envelope, "correct", PUBLIC).unwrap();
        let signer = unlock_software_signer(&registry, &envelope, &key, PUBLIC).unwrap();

        assert_eq!(signer.public_key(), PUBLIC);
    }

    #[test]
    fn wrong_passcode_cannot_derive_verified_unlock_key() {
        let registry = ProtectionRegistry::new();
        let envelope = protected_secret(&registry, "correct");

        let error = derive_verified_unlock_key(&registry, &envelope, "wrong", PUBLIC).unwrap_err();

        assert_eq!(
            error,
            ProtectedSignerError::Protection(ProtectionError::SecretStore(
                SecretStoreError::InvalidPassword
            ))
        );
    }

    #[test]
    fn derived_unlock_key_is_bound_to_expected_identity() {
        let registry = ProtectionRegistry::new();
        let envelope = protected_secret(&registry, "correct");

        let error = derive_verified_unlock_key(
            &registry,
            &envelope,
            "correct",
            "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY",
        )
        .unwrap_err();

        assert_eq!(error, ProtectedSignerError::IdentityMismatch);
    }

    #[test]
    fn signs_protected_transaction_with_unlock_key() {
        let registry = ProtectionRegistry::new();
        let protected = protected_secret(&registry, "correct");
        let unlock_key =
            derive_verified_unlock_key(&registry, &protected, "correct", PUBLIC).unwrap();
        let mut transaction =
            crate::parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();

        sign_protected_transaction_envelope(
            &registry,
            &protected,
            &unlock_key,
            PUBLIC,
            &mut transaction,
            TESTNET,
        )
        .unwrap();

        assert_eq!(
            crate::transaction_envelope_xdr(&transaction).unwrap(),
            decode_hex(SIGNED_XDR_HEX)
        );
    }

    #[test]
    fn wrong_unlock_key_cannot_sign() {
        let registry = ProtectionRegistry::new();
        let protected = protected_secret(&registry, "correct");
        let wrong = WalletUnlockKey::from_bytes([0u8; 32]);
        let mut transaction =
            crate::parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();

        let error = sign_protected_transaction_envelope(
            &registry,
            &protected,
            &wrong,
            PUBLIC,
            &mut transaction,
            TESTNET,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProtectedSigningError::Unlock(ProtectedSignerError::Protection(
                ProtectionError::SecretStore(SecretStoreError::InvalidUnlockKey)
            ))
        ));
    }

    #[test]
    fn exports_secret_with_fresh_passcode() {
        let registry = ProtectionRegistry::new();
        let envelope = protected_secret(&registry, "correct");

        let material = export_signing_material(&registry, &envelope, "correct", PUBLIC).unwrap();

        match material {
            ExportedSigningMaterial::Secret { secret } => assert_eq!(secret.as_str(), SECRET),
            ExportedSigningMaterial::Mnemonic { .. } => panic!("expected secret export"),
        }
    }

    #[test]
    fn wrong_passcode_cannot_export() {
        let registry = ProtectionRegistry::new();
        let envelope = protected_secret(&registry, "correct");

        let error = export_signing_material(&registry, &envelope, "wrong", PUBLIC).unwrap_err();

        assert_eq!(
            error,
            ProtectedSignerError::Protection(ProtectionError::SecretStore(
                SecretStoreError::InvalidPassword
            ))
        );
    }

    #[test]
    fn exports_mnemonic_with_reconstruction_metadata() {
        let registry = ProtectionRegistry::new();
        let envelope = registry
            .protect(
                &json!({
                    "kind": "mnemonic",
                    "mnemonic": CHINESE_MNEMONIC,
                    "mnemonic_passphrase": "",
                    "index": 0
                }),
                &ProtectionCredential::password("correct"),
            )
            .unwrap();

        let material =
            export_signing_material(&registry, &envelope, "correct", CHINESE_PUBLIC).unwrap();

        match material {
            ExportedSigningMaterial::Mnemonic {
                mnemonic,
                mnemonic_passphrase,
                index,
                language,
            } => {
                assert_eq!(mnemonic.as_str(), CHINESE_MNEMONIC);
                assert_eq!(mnemonic_passphrase.as_str(), "");
                assert_eq!(index, 0);
                assert_eq!(language, "chinese_simplified");
            }
            ExportedSigningMaterial::Secret { .. } => panic!("expected mnemonic export"),
        }
    }

    #[test]
    fn exported_material_debug_redacts_secret_values() {
        let material = ExportedSigningMaterial::Secret {
            secret: Zeroizing::new(SECRET.to_owned()),
        };
        let debug = format!("{material:?}");

        assert!(!debug.contains(SECRET));
        assert!(debug.contains("<redacted>"));
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
    fn rejects_unknown_signing_material_kind() {
        let error = software_signer_from_payload(json!({
            "kind": "future",
            "secret": SECRET,
            "mnemonic": CHINESE_MNEMONIC,
            "mnemonic_passphrase": "sensitive"
        }))
        .err()
        .unwrap();

        assert_eq!(error, ProtectedSignerError::UnsupportedSigningMaterial);
    }
}

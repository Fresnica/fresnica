//! Mobile UniFFI compatibility facade over `fresnica_sdk::FresnicaSdk`.
//!
//! The v0.1.0 Swift/Kotlin-facing surface remains stable while the semantic
//! wallet/signing contract lives in the platform-neutral SDK. This crate owns
//! only UniFFI DTO/error translation; it does not reproduce Core cryptography,
//! signer identity checks, envelope parsing, unlock-key validation, transaction
//! hashing, or signature verification.

use std::{error::Error, fmt};

use fresnica_sdk::{
    FresnicaSdk, SdkAccountKind, SdkError, SdkErrorCode, SdkExportedSigningMaterial,
    SdkGeneratedMnemonic, SdkProtectedSoftwareSigner, SdkSigningMaterialKind,
};
use serde::{Deserialize, Serialize};

uniffi::setup_scaffolding!();

pub const MOBILE_BINDING_API_VERSION: u64 = 2;

/// Stateless compatibility entry point for the existing Mobile v0.1.0 API.
#[derive(uniffi::Object)]
pub struct MobileCoreApi;

impl Default for MobileCoreApi {
    fn default() -> Self {
        Self::new()
    }
}

impl MobileCoreApi {
    fn sdk(&self) -> FresnicaSdk {
        FresnicaSdk::new()
    }
}

#[uniffi::export]
impl MobileCoreApi {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self
    }

    pub fn version(&self) -> MobileCoreVersion {
        let sdk_version = self.sdk().version();
        MobileCoreVersion {
            mobile_binding_api_version: MOBILE_BINDING_API_VERSION,
            core_client_api_version: sdk_version.core_client_api_version,
        }
    }

    pub fn parse_account(&self, address: String) -> Result<MobileAccountIdentity, MobileCoreError> {
        let identity = self
            .sdk()
            .parse_account(address)
            .map_err(MobileCoreError::from)?;
        Ok(MobileAccountIdentity {
            kind: match identity.kind {
                SdkAccountKind::Classic => MobileAccountKind::Classic,
                SdkAccountKind::Contract => MobileAccountKind::Contract,
            },
            address: identity.address,
            public_key: identity.public_key,
        })
    }

    pub fn protect_secret(
        &self,
        secret: String,
        passcode: String,
        expected_signer_public_key: Option<String>,
    ) -> Result<MobileProtectedSoftwareSigner, MobileCoreError> {
        self.sdk()
            .protect_secret(secret, passcode, expected_signer_public_key)
            .map(mobile_protected_signer)
            .map_err(MobileCoreError::from)
    }

    pub fn protect_mnemonic(
        &self,
        mnemonic: String,
        mnemonic_passphrase: String,
        index: u32,
        language: Option<String>,
        passcode: String,
        expected_signer_public_key: Option<String>,
    ) -> Result<MobileProtectedSoftwareSigner, MobileCoreError> {
        self.sdk()
            .protect_mnemonic(
                mnemonic,
                mnemonic_passphrase,
                index,
                language,
                passcode,
                expected_signer_public_key,
            )
            .map(mobile_protected_signer)
            .map_err(MobileCoreError::from)
    }

    pub fn generate_mnemonic(
        &self,
        language: String,
        strength: u32,
        mnemonic_passphrase: String,
        index: u32,
        passcode: String,
    ) -> Result<MobileGeneratedMnemonic, MobileCoreError> {
        self.sdk()
            .generate_mnemonic(language, strength, mnemonic_passphrase, index, passcode)
            .map(mobile_generated_mnemonic)
            .map_err(MobileCoreError::from)
    }

    pub fn reprotect(
        &self,
        envelope_json: String,
        current_passcode: String,
        new_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<MobileProtectedSoftwareSigner, MobileCoreError> {
        self.sdk()
            .reprotect(
                envelope_json,
                current_passcode,
                new_passcode,
                expected_signer_public_key,
            )
            .map(mobile_protected_signer)
            .map_err(MobileCoreError::from)
    }

    pub fn derive_unlock_key(
        &self,
        envelope_json: String,
        passcode: String,
        expected_signer_public_key: String,
    ) -> Result<Vec<u8>, MobileCoreError> {
        self.sdk()
            .derive_unlock_key(envelope_json, passcode, expected_signer_public_key)
            .map_err(MobileCoreError::from)
    }

    pub fn validate_unlock_key(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
    ) -> Result<(), MobileCoreError> {
        self.sdk()
            .validate_unlock_key(envelope_json, unlock_key, expected_signer_public_key)
            .map_err(MobileCoreError::from)
    }

    pub fn sign_transaction_xdr(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<Vec<u8>, MobileCoreError> {
        self.sdk()
            .sign_transaction_xdr(
                envelope_json,
                unlock_key,
                expected_signer_public_key,
                transaction_xdr,
                network_passphrase,
            )
            .map_err(MobileCoreError::from)
    }

    pub fn reveal(
        &self,
        envelope_json: String,
        fresh_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<MobileExportedSigningMaterial, MobileCoreError> {
        self.sdk()
            .reveal(envelope_json, fresh_passcode, expected_signer_public_key)
            .map(mobile_exported_signing_material)
            .map_err(MobileCoreError::from)
    }

    pub fn prepare_ed25519_signing(
        &self,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<MobileEd25519SigningRequest, MobileCoreError> {
        let request = self
            .sdk()
            .prepare_ed25519_signing(transaction_xdr, network_passphrase)
            .map_err(MobileCoreError::from)?;
        Ok(MobileEd25519SigningRequest {
            transaction_hash: request.transaction_hash,
            transaction_xdr: request.transaction_xdr,
            network_passphrase: request.network_passphrase,
        })
    }

    pub fn apply_ed25519_signature(
        &self,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
        signer_public_key: String,
        signature: Vec<u8>,
    ) -> Result<Vec<u8>, MobileCoreError> {
        self.sdk()
            .apply_ed25519_signature(
                transaction_xdr,
                network_passphrase,
                signer_public_key,
                signature,
            )
            .map_err(MobileCoreError::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct MobileCoreVersion {
    pub mobile_binding_api_version: u64,
    pub core_client_api_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum MobileAccountKind {
    Classic,
    Contract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct MobileAccountIdentity {
    pub kind: MobileAccountKind,
    pub address: String,
    pub public_key: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct MobileProtectedSoftwareSigner {
    pub signer_public_key: String,
    pub envelope_json: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct MobileGeneratedMnemonic {
    pub signer: MobileProtectedSoftwareSigner,
    pub mnemonic: String,
    pub language: String,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct MobileEd25519SigningRequest {
    pub transaction_hash: Vec<u8>,
    pub transaction_xdr: Vec<u8>,
    pub network_passphrase: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum MobileSigningMaterialKind {
    Secret,
    Mnemonic,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct MobileExportedSigningMaterial {
    pub kind: MobileSigningMaterialKind,
    pub secret: Option<String>,
    pub mnemonic: Option<String>,
    pub mnemonic_passphrase: Option<String>,
    pub index: Option<u32>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum MobileCoreErrorCode {
    InvalidInput,
    InvalidPasscode,
    InvalidUnlockKey,
    InvalidProtectedData,
    IdentityMismatch,
    InvalidTransaction,
    CoreError,
}

impl MobileCoreErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid-input",
            Self::InvalidPasscode => "invalid-passcode",
            Self::InvalidUnlockKey => "invalid-unlock-key",
            Self::InvalidProtectedData => "invalid-protected-data",
            Self::IdentityMismatch => "identity-mismatch",
            Self::InvalidTransaction => "invalid-transaction",
            Self::CoreError => "core-error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Error)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum MobileCoreError {
    InvalidInput { detail: String },
    InvalidPasscode { detail: String },
    InvalidUnlockKey { detail: String },
    InvalidProtectedData { detail: String },
    IdentityMismatch { detail: String },
    InvalidTransaction { detail: String },
    CoreError { detail: String },
}

impl MobileCoreError {
    pub fn code(&self) -> MobileCoreErrorCode {
        match self {
            Self::InvalidInput { .. } => MobileCoreErrorCode::InvalidInput,
            Self::InvalidPasscode { .. } => MobileCoreErrorCode::InvalidPasscode,
            Self::InvalidUnlockKey { .. } => MobileCoreErrorCode::InvalidUnlockKey,
            Self::InvalidProtectedData { .. } => MobileCoreErrorCode::InvalidProtectedData,
            Self::IdentityMismatch { .. } => MobileCoreErrorCode::IdentityMismatch,
            Self::InvalidTransaction { .. } => MobileCoreErrorCode::InvalidTransaction,
            Self::CoreError { .. } => MobileCoreErrorCode::CoreError,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::InvalidInput { detail }
            | Self::InvalidPasscode { detail }
            | Self::InvalidUnlockKey { detail }
            | Self::InvalidProtectedData { detail }
            | Self::IdentityMismatch { detail }
            | Self::InvalidTransaction { detail }
            | Self::CoreError { detail } => detail,
        }
    }

    fn new(code: MobileCoreErrorCode, message: impl Into<String>) -> Self {
        let detail = message.into();
        match code {
            MobileCoreErrorCode::InvalidInput => Self::InvalidInput { detail },
            MobileCoreErrorCode::InvalidPasscode => Self::InvalidPasscode { detail },
            MobileCoreErrorCode::InvalidUnlockKey => Self::InvalidUnlockKey { detail },
            MobileCoreErrorCode::InvalidProtectedData => Self::InvalidProtectedData { detail },
            MobileCoreErrorCode::IdentityMismatch => Self::IdentityMismatch { detail },
            MobileCoreErrorCode::InvalidTransaction => Self::InvalidTransaction { detail },
            MobileCoreErrorCode::CoreError => Self::CoreError { detail },
        }
    }
}

impl fmt::Display for MobileCoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl Error for MobileCoreError {}

impl From<SdkError> for MobileCoreError {
    fn from(error: SdkError) -> Self {
        let code = match error.code {
            SdkErrorCode::InvalidInput => MobileCoreErrorCode::InvalidInput,
            SdkErrorCode::InvalidPasscode => MobileCoreErrorCode::InvalidPasscode,
            SdkErrorCode::InvalidUnlockKey => MobileCoreErrorCode::InvalidUnlockKey,
            SdkErrorCode::InvalidProtectedData => MobileCoreErrorCode::InvalidProtectedData,
            SdkErrorCode::IdentityMismatch => MobileCoreErrorCode::IdentityMismatch,
            SdkErrorCode::InvalidTransaction => MobileCoreErrorCode::InvalidTransaction,
            SdkErrorCode::CoreError => MobileCoreErrorCode::CoreError,
            _ => MobileCoreErrorCode::CoreError,
        };
        Self::new(code, error.message)
    }
}

fn mobile_protected_signer(
    protected: SdkProtectedSoftwareSigner,
) -> MobileProtectedSoftwareSigner {
    MobileProtectedSoftwareSigner {
        signer_public_key: protected.signer_public_key,
        envelope_json: protected.envelope_json,
    }
}

fn mobile_generated_mnemonic(generated: SdkGeneratedMnemonic) -> MobileGeneratedMnemonic {
    MobileGeneratedMnemonic {
        signer: mobile_protected_signer(generated.signer),
        mnemonic: generated.mnemonic,
        language: generated.language,
        index: generated.index,
    }
}

fn mobile_exported_signing_material(
    material: SdkExportedSigningMaterial,
) -> MobileExportedSigningMaterial {
    MobileExportedSigningMaterial {
        kind: match material.kind {
            SdkSigningMaterialKind::Secret => MobileSigningMaterialKind::Secret,
            SdkSigningMaterialKind::Mnemonic => MobileSigningMaterialKind::Mnemonic,
        },
        secret: material.secret,
        mnemonic: material.mnemonic,
        mnemonic_passphrase: material.mnemonic_passphrase,
        index: material.index,
        language: material.language,
    }
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::Value;

    use super::*;

    const CLASSIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const OTHER_CLASSIC: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";
    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
    const MNEMONIC: &str =
        "illness spike retreat truth genius clock brain pass fit cave bargain toe";
    const MNEMONIC_PUBLIC: &str = "GDRXE2BQUC3AZNPVFSCEZ76NJ3WWL25FYFK6RGZGIEKWE4SOOHSUJUJ6";

    fn signing_vector() -> Value {
        serde_json::from_str(include_str!(
            "../../../spec/test-vectors/transaction-signing-v1.json"
        ))
        .unwrap()
    }

    fn decode_hex(text: &str) -> Vec<u8> {
        assert_eq!(text.len() % 2, 0);
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn mobile_api_is_safe_for_foreign_threading() {
        assert_send_sync::<MobileCoreApi>();
    }

    #[test]
    fn versions_and_account_identity_are_stable() {
        let api = MobileCoreApi::new();
        let version = api.version();
        assert_eq!(
            version.mobile_binding_api_version,
            MOBILE_BINDING_API_VERSION
        );
        assert_eq!(
            version.core_client_api_version,
            FresnicaSdk::new().version().core_client_api_version
        );

        let classic = api.parse_account(CLASSIC.to_owned()).unwrap();
        assert_eq!(classic.kind, MobileAccountKind::Classic);
        assert_eq!(classic.address, CLASSIC);
        assert_eq!(classic.public_key.as_deref(), Some(CLASSIC));

        let contract = api.parse_account(CONTRACT.to_owned()).unwrap();
        assert_eq!(contract.kind, MobileAccountKind::Contract);
        assert_eq!(contract.address, CONTRACT);
        assert_eq!(contract.public_key, None);
    }

    #[test]
    fn software_signer_binding_roundtrips_shared_transaction_vector() {
        let vector = signing_vector();
        let case = &vector["cases"][0];
        let secret = case["secret"].as_str().unwrap();
        let public_key = case["public_key"].as_str().unwrap();
        let network = case["network_passphrase"].as_str().unwrap();
        let unsigned = STANDARD
            .decode(case["unsigned_xdr_base64"].as_str().unwrap())
            .unwrap();
        let expected_signed = STANDARD
            .decode(case["signed_xdr_base64"].as_str().unwrap())
            .unwrap();

        let api = MobileCoreApi::new();
        let protected = api
            .protect_secret(
                secret.to_owned(),
                "old-passcode".to_owned(),
                Some(public_key.to_owned()),
            )
            .unwrap();
        assert_eq!(protected.signer_public_key, public_key);
        assert!(!protected.envelope_json.contains(secret));

        let unlock_key = api
            .derive_unlock_key(
                protected.envelope_json.clone(),
                "old-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        assert_eq!(unlock_key.len(), 32);
        api.validate_unlock_key(
            protected.envelope_json.clone(),
            unlock_key.clone(),
            public_key.to_owned(),
        )
        .unwrap();

        let signed = api
            .sign_transaction_xdr(
                protected.envelope_json.clone(),
                unlock_key.clone(),
                public_key.to_owned(),
                unsigned,
                network.to_owned(),
            )
            .unwrap();
        assert_eq!(signed, expected_signed);

        let revealed = api
            .reveal(
                protected.envelope_json.clone(),
                "old-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        assert_eq!(revealed.kind, MobileSigningMaterialKind::Secret);
        assert_eq!(revealed.secret.as_deref(), Some(secret));

        let reprotected = api
            .reprotect(
                protected.envelope_json,
                "old-passcode".to_owned(),
                "new-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        assert_eq!(reprotected.signer_public_key, public_key);

        let old_error = api
            .derive_unlock_key(
                reprotected.envelope_json.clone(),
                "old-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap_err();
        assert_eq!(old_error.code(), MobileCoreErrorCode::InvalidPasscode);

        let new_key = api
            .derive_unlock_key(
                reprotected.envelope_json,
                "new-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        assert_ne!(new_key, unlock_key);
    }

    #[test]
    fn mnemonic_import_generation_and_reveal_stay_on_same_facade() {
        let api = MobileCoreApi::new();
        let imported = api
            .protect_mnemonic(
                MNEMONIC.to_owned(),
                String::new(),
                0,
                Some("english".to_owned()),
                "passcode".to_owned(),
                Some(MNEMONIC_PUBLIC.to_owned()),
            )
            .unwrap();
        assert_eq!(imported.signer_public_key, MNEMONIC_PUBLIC);

        let generated = api
            .generate_mnemonic(
                "english".to_owned(),
                128,
                String::new(),
                0,
                "passcode".to_owned(),
            )
            .unwrap();
        assert!(!generated.mnemonic.is_empty());
        assert_eq!(generated.language, "english");

        let revealed = api
            .reveal(
                generated.signer.envelope_json,
                "passcode".to_owned(),
                generated.signer.signer_public_key,
            )
            .unwrap();
        assert_eq!(revealed.kind, MobileSigningMaterialKind::Mnemonic);
        assert_eq!(
            revealed.mnemonic.as_deref(),
            Some(generated.mnemonic.as_str())
        );
        assert_eq!(revealed.index, Some(0));
        assert_eq!(revealed.language.as_deref(), Some("english"));
    }

    #[test]
    fn signer_attachment_and_boundary_errors_keep_stable_codes() {
        let vector = signing_vector();
        let secret = vector["cases"][0]["secret"].as_str().unwrap();
        let api = MobileCoreApi::new();

        let mismatch = api
            .protect_secret(
                secret.to_owned(),
                "passcode".to_owned(),
                Some(OTHER_CLASSIC.to_owned()),
            )
            .err()
            .unwrap();
        assert_eq!(mismatch.code(), MobileCoreErrorCode::IdentityMismatch);
        assert_eq!(
            serde_json::to_value(&mismatch).unwrap()["code"],
            "identity-mismatch"
        );

        let malformed = api
            .derive_unlock_key(
                "not-json".to_owned(),
                "passcode".to_owned(),
                CLASSIC.to_owned(),
            )
            .unwrap_err();
        assert_eq!(malformed.code(), MobileCoreErrorCode::InvalidProtectedData);

        let protected = api
            .protect_secret(secret.to_owned(), "passcode".to_owned(), None)
            .unwrap();
        let bad_key = api
            .validate_unlock_key(protected.envelope_json, vec![0u8; 31], CLASSIC.to_owned())
            .unwrap_err();
        assert_eq!(bad_key.code(), MobileCoreErrorCode::InvalidUnlockKey);
    }

    #[test]
    fn external_ed25519_binding_roundtrips_shared_vector() {
        let vector = signing_vector();
        let case = &vector["cases"][0];
        let public_key = case["public_key"].as_str().unwrap();
        let network = case["network_passphrase"].as_str().unwrap();
        let unsigned = STANDARD
            .decode(case["unsigned_xdr_base64"].as_str().unwrap())
            .unwrap();
        let expected_hash = decode_hex(case["transaction_hash_hex"].as_str().unwrap());
        let signature = decode_hex(case["signature_hex"].as_str().unwrap());
        let expected_signed = STANDARD
            .decode(case["signed_xdr_base64"].as_str().unwrap())
            .unwrap();

        let api = MobileCoreApi::new();
        let request = api
            .prepare_ed25519_signing(unsigned.clone(), network.to_owned())
            .unwrap();
        assert_eq!(request.transaction_hash, expected_hash);
        assert_eq!(request.transaction_xdr, unsigned);
        assert_eq!(request.network_passphrase, network);

        let signed = api
            .apply_ed25519_signature(
                request.transaction_xdr,
                network.to_owned(),
                public_key.to_owned(),
                signature,
            )
            .unwrap();
        assert_eq!(signed, expected_signed);
    }
}

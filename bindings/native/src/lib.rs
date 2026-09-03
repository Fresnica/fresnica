//! Native-platform UniFFI binding over the platform-neutral `fresnica-sdk`.
//!
//! This crate is mechanical FFI glue for direct native SDK consumers. Wallet,
//! signer, protection, transaction, and error semantics remain owned by
//! `fresnica-sdk` and Rust Core. Framework-specific adapters do not belong here.

use std::{error::Error, fmt};

use fresnica_sdk::{
    FresnicaSdk, SdkAccountKind, SdkError, SdkErrorCode, SdkExportedSigningMaterial,
    SdkGeneratedMnemonic, SdkProtectedSoftwareSigner, SdkSigningMaterialKind,
};
use serde::{Deserialize, Serialize};

uniffi::setup_scaffolding!();

pub const NATIVE_BINDING_API_VERSION: u64 = 3;

#[derive(uniffi::Object)]
pub struct FresnicaSdkApi;

impl Default for FresnicaSdkApi {
    fn default() -> Self {
        Self::new()
    }
}

impl FresnicaSdkApi {
    fn sdk(&self) -> FresnicaSdk {
        FresnicaSdk::new()
    }
}

#[uniffi::export]
impl FresnicaSdkApi {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self
    }

    pub fn version(&self) -> NativeSdkVersion {
        let version = self.sdk().version();
        NativeSdkVersion {
            native_binding_api_version: NATIVE_BINDING_API_VERSION,
            sdk_api_version: version.sdk_api_version,
            core_client_api_version: version.core_client_api_version,
        }
    }

    pub fn parse_account(&self, address: String) -> Result<NativeAccountIdentity, NativeSdkError> {
        let identity = self
            .sdk()
            .parse_account(address)
            .map_err(NativeSdkError::from)?;
        Ok(NativeAccountIdentity {
            kind: match identity.kind {
                SdkAccountKind::Classic => NativeAccountKind::Classic,
                SdkAccountKind::Contract => NativeAccountKind::Contract,
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
    ) -> Result<NativeProtectedSoftwareSigner, NativeSdkError> {
        self.sdk()
            .protect_secret(secret, passcode, expected_signer_public_key)
            .map(native_protected_signer)
            .map_err(NativeSdkError::from)
    }

    pub fn protect_mnemonic(
        &self,
        mnemonic: String,
        mnemonic_passphrase: String,
        index: u32,
        language: Option<String>,
        passcode: String,
        expected_signer_public_key: Option<String>,
    ) -> Result<NativeProtectedSoftwareSigner, NativeSdkError> {
        self.sdk()
            .protect_mnemonic(
                mnemonic,
                mnemonic_passphrase,
                index,
                language,
                passcode,
                expected_signer_public_key,
            )
            .map(native_protected_signer)
            .map_err(NativeSdkError::from)
    }

    pub fn generate_mnemonic(
        &self,
        language: String,
        strength: u32,
        mnemonic_passphrase: String,
        index: u32,
        passcode: String,
    ) -> Result<NativeGeneratedMnemonic, NativeSdkError> {
        self.sdk()
            .generate_mnemonic(language, strength, mnemonic_passphrase, index, passcode)
            .map(native_generated_mnemonic)
            .map_err(NativeSdkError::from)
    }

    pub fn derive_mnemonic_signer(
        &self,
        source_envelope_json: String,
        app_passcode: String,
        expected_source_signer_public_key: String,
        index: u32,
    ) -> Result<NativeProtectedSoftwareSigner, NativeSdkError> {
        self.sdk()
            .derive_mnemonic_signer(
                source_envelope_json,
                app_passcode,
                expected_source_signer_public_key,
                index,
            )
            .map(native_protected_signer)
            .map_err(NativeSdkError::from)
    }

    pub fn reprotect(
        &self,
        envelope_json: String,
        current_passcode: String,
        new_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<NativeProtectedSoftwareSigner, NativeSdkError> {
        self.sdk()
            .reprotect(
                envelope_json,
                current_passcode,
                new_passcode,
                expected_signer_public_key,
            )
            .map(native_protected_signer)
            .map_err(NativeSdkError::from)
    }

    /// Native-only routine signing key material. Framework adapters must not
    /// forward this byte array into JavaScript/Dart convenience APIs.
    pub fn derive_unlock_key(
        &self,
        envelope_json: String,
        passcode: String,
        expected_signer_public_key: String,
    ) -> Result<Vec<u8>, NativeSdkError> {
        self.sdk()
            .derive_unlock_key(envelope_json, passcode, expected_signer_public_key)
            .map_err(NativeSdkError::from)
    }

    pub fn validate_unlock_key(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
    ) -> Result<(), NativeSdkError> {
        self.sdk()
            .validate_unlock_key(envelope_json, unlock_key, expected_signer_public_key)
            .map_err(NativeSdkError::from)
    }

    pub fn sign_transaction_xdr(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<Vec<u8>, NativeSdkError> {
        self.sdk()
            .sign_transaction_xdr(
                envelope_json,
                unlock_key,
                expected_signer_public_key,
                transaction_xdr,
                network_passphrase,
            )
            .map_err(NativeSdkError::from)
    }

    pub fn sign_message(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
        message: Vec<u8>,
    ) -> Result<Vec<u8>, NativeSdkError> {
        self.sdk()
            .sign_message(
                envelope_json,
                unlock_key,
                expected_signer_public_key,
                message,
            )
            .map_err(NativeSdkError::from)
    }

    pub fn sign_message_with_passcode(
        &self,
        envelope_json: String,
        app_passcode: String,
        expected_signer_public_key: String,
        message: Vec<u8>,
    ) -> Result<Vec<u8>, NativeSdkError> {
        self.sdk()
            .sign_message_with_passcode(
                envelope_json,
                app_passcode,
                expected_signer_public_key,
                message,
            )
            .map_err(NativeSdkError::from)
    }

    pub fn reveal(
        &self,
        envelope_json: String,
        fresh_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<NativeExportedSigningMaterial, NativeSdkError> {
        self.sdk()
            .reveal(envelope_json, fresh_passcode, expected_signer_public_key)
            .map(native_exported_signing_material)
            .map_err(NativeSdkError::from)
    }

    pub fn prepare_ed25519_signing(
        &self,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<NativeEd25519SigningRequest, NativeSdkError> {
        let request = self
            .sdk()
            .prepare_ed25519_signing(transaction_xdr, network_passphrase)
            .map_err(NativeSdkError::from)?;
        Ok(NativeEd25519SigningRequest {
            transaction_hash: request.transaction_hash,
            transaction_xdr: request.transaction_xdr,
            network_passphrase: request.network_passphrase,
        })
    }

    pub fn prepare_message_signing(&self, message: Vec<u8>) -> NativeMessageSigningRequest {
        let request = self.sdk().prepare_message_signing(message);
        NativeMessageSigningRequest {
            message_hash: request.message_hash,
            message: request.message,
            encoded_message: request.encoded_message,
        }
    }

    pub fn verify_message_signature(
        &self,
        message: Vec<u8>,
        signer_public_key: String,
        signature: Vec<u8>,
    ) -> Result<(), NativeSdkError> {
        self.sdk()
            .verify_message_signature(message, signer_public_key, signature)
            .map_err(NativeSdkError::from)
    }

    pub fn apply_ed25519_signature(
        &self,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
        signer_public_key: String,
        signature: Vec<u8>,
    ) -> Result<Vec<u8>, NativeSdkError> {
        self.sdk()
            .apply_ed25519_signature(
                transaction_xdr,
                network_passphrase,
                signer_public_key,
                signature,
            )
            .map_err(NativeSdkError::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct NativeSdkVersion {
    pub native_binding_api_version: u64,
    pub sdk_api_version: u64,
    pub core_client_api_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum NativeAccountKind {
    Classic,
    Contract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct NativeAccountIdentity {
    pub kind: NativeAccountKind,
    pub address: String,
    pub public_key: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct NativeProtectedSoftwareSigner {
    pub signer_public_key: String,
    pub envelope_json: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct NativeGeneratedMnemonic {
    pub signer: NativeProtectedSoftwareSigner,
    pub mnemonic: String,
    pub language: String,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct NativeEd25519SigningRequest {
    pub transaction_hash: Vec<u8>,
    pub transaction_xdr: Vec<u8>,
    pub network_passphrase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct NativeMessageSigningRequest {
    pub message_hash: Vec<u8>,
    pub message: Vec<u8>,
    pub encoded_message: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum NativeSigningMaterialKind {
    Secret,
    Mnemonic,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct NativeExportedSigningMaterial {
    pub kind: NativeSigningMaterialKind,
    pub secret: Option<String>,
    pub mnemonic: Option<String>,
    pub mnemonic_passphrase: Option<String>,
    pub index: Option<u32>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum NativeSdkErrorCode {
    InvalidInput,
    InvalidPasscode,
    InvalidUnlockKey,
    InvalidProtectedData,
    IdentityMismatch,
    InvalidTransaction,
    InvalidMessageSignature,
    CoreError,
}

impl NativeSdkErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid-input",
            Self::InvalidPasscode => "invalid-passcode",
            Self::InvalidUnlockKey => "invalid-unlock-key",
            Self::InvalidProtectedData => "invalid-protected-data",
            Self::IdentityMismatch => "identity-mismatch",
            Self::InvalidTransaction => "invalid-transaction",
            Self::InvalidMessageSignature => "invalid-message-signature",
            Self::CoreError => "core-error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Error)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum NativeSdkError {
    InvalidInput { detail: String },
    InvalidPasscode { detail: String },
    InvalidUnlockKey { detail: String },
    InvalidProtectedData { detail: String },
    IdentityMismatch { detail: String },
    InvalidTransaction { detail: String },
    InvalidMessageSignature { detail: String },
    CoreError { detail: String },
}

impl NativeSdkError {
    pub fn code(&self) -> NativeSdkErrorCode {
        match self {
            Self::InvalidInput { .. } => NativeSdkErrorCode::InvalidInput,
            Self::InvalidPasscode { .. } => NativeSdkErrorCode::InvalidPasscode,
            Self::InvalidUnlockKey { .. } => NativeSdkErrorCode::InvalidUnlockKey,
            Self::InvalidProtectedData { .. } => NativeSdkErrorCode::InvalidProtectedData,
            Self::IdentityMismatch { .. } => NativeSdkErrorCode::IdentityMismatch,
            Self::InvalidTransaction { .. } => NativeSdkErrorCode::InvalidTransaction,
            Self::InvalidMessageSignature { .. } => NativeSdkErrorCode::InvalidMessageSignature,
            Self::CoreError { .. } => NativeSdkErrorCode::CoreError,
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
            | Self::InvalidMessageSignature { detail }
            | Self::CoreError { detail } => detail,
        }
    }

    fn new(code: NativeSdkErrorCode, message: impl Into<String>) -> Self {
        let detail = message.into();
        match code {
            NativeSdkErrorCode::InvalidInput => Self::InvalidInput { detail },
            NativeSdkErrorCode::InvalidPasscode => Self::InvalidPasscode { detail },
            NativeSdkErrorCode::InvalidUnlockKey => Self::InvalidUnlockKey { detail },
            NativeSdkErrorCode::InvalidProtectedData => Self::InvalidProtectedData { detail },
            NativeSdkErrorCode::IdentityMismatch => Self::IdentityMismatch { detail },
            NativeSdkErrorCode::InvalidTransaction => Self::InvalidTransaction { detail },
            NativeSdkErrorCode::InvalidMessageSignature => Self::InvalidMessageSignature { detail },
            NativeSdkErrorCode::CoreError => Self::CoreError { detail },
        }
    }
}

impl fmt::Display for NativeSdkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl Error for NativeSdkError {}

impl From<SdkError> for NativeSdkError {
    fn from(error: SdkError) -> Self {
        let code = match error.code {
            SdkErrorCode::InvalidInput => NativeSdkErrorCode::InvalidInput,
            SdkErrorCode::InvalidPasscode => NativeSdkErrorCode::InvalidPasscode,
            SdkErrorCode::InvalidUnlockKey => NativeSdkErrorCode::InvalidUnlockKey,
            SdkErrorCode::InvalidProtectedData => NativeSdkErrorCode::InvalidProtectedData,
            SdkErrorCode::IdentityMismatch => NativeSdkErrorCode::IdentityMismatch,
            SdkErrorCode::InvalidTransaction => NativeSdkErrorCode::InvalidTransaction,
            SdkErrorCode::InvalidMessageSignature => NativeSdkErrorCode::InvalidMessageSignature,
            SdkErrorCode::CoreError => NativeSdkErrorCode::CoreError,
            _ => NativeSdkErrorCode::CoreError,
        };
        Self::new(code, error.message)
    }
}

fn native_protected_signer(protected: SdkProtectedSoftwareSigner) -> NativeProtectedSoftwareSigner {
    NativeProtectedSoftwareSigner {
        signer_public_key: protected.signer_public_key,
        envelope_json: protected.envelope_json,
    }
}

fn native_generated_mnemonic(generated: SdkGeneratedMnemonic) -> NativeGeneratedMnemonic {
    NativeGeneratedMnemonic {
        signer: native_protected_signer(generated.signer),
        mnemonic: generated.mnemonic,
        language: generated.language,
        index: generated.index,
    }
}

fn native_exported_signing_material(
    material: SdkExportedSigningMaterial,
) -> NativeExportedSigningMaterial {
    NativeExportedSigningMaterial {
        kind: match material.kind {
            SdkSigningMaterialKind::Secret => NativeSigningMaterialKind::Secret,
            SdkSigningMaterialKind::Mnemonic => NativeSigningMaterialKind::Mnemonic,
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

    fn signing_vector() -> Value {
        serde_json::from_str(include_str!(
            "../../../spec/test-vectors/transaction-signing-v1.json"
        ))
        .unwrap()
    }

    fn message_signing_vector() -> Value {
        serde_json::from_str(include_str!(
            "../../../spec/test-vectors/message-signing-v1.json"
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

    #[test]
    fn versions_and_account_identity_are_native_sdk_stable() {
        let api = FresnicaSdkApi::new();
        let version = api.version();
        let sdk_version = FresnicaSdk::new().version();
        assert_eq!(
            version.native_binding_api_version,
            NATIVE_BINDING_API_VERSION
        );
        assert_eq!(version.sdk_api_version, sdk_version.sdk_api_version);
        assert_eq!(
            version.core_client_api_version,
            sdk_version.core_client_api_version
        );

        let classic = api.parse_account(CLASSIC.to_owned()).unwrap();
        assert_eq!(classic.kind, NativeAccountKind::Classic);
        assert_eq!(classic.public_key.as_deref(), Some(CLASSIC));

        let contract = api.parse_account(CONTRACT.to_owned()).unwrap();
        assert_eq!(contract.kind, NativeAccountKind::Contract);
        assert_eq!(contract.public_key, None);
    }

    #[test]
    fn derives_mnemonic_signer_through_native_binding() {
        let api = FresnicaSdkApi::new();
        let generated = api
            .generate_mnemonic(
                "english".to_owned(),
                128,
                String::new(),
                0,
                "passcode".to_owned(),
            )
            .unwrap();
        let source_public_key = generated.signer.signer_public_key.clone();
        let derived = api
            .derive_mnemonic_signer(
                generated.signer.envelope_json,
                "passcode".to_owned(),
                source_public_key.clone(),
                1,
            )
            .unwrap();
        assert_ne!(derived.signer_public_key, source_public_key);
    }

    #[test]
    fn protected_signing_roundtrips_shared_vector() {
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

        let api = FresnicaSdkApi::new();
        let protected = api
            .protect_secret(
                secret.to_owned(),
                "passcode".to_owned(),
                Some(public_key.to_owned()),
            )
            .unwrap();
        let unlock_key = api
            .derive_unlock_key(
                protected.envelope_json.clone(),
                "passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        let signed = api
            .sign_transaction_xdr(
                protected.envelope_json,
                unlock_key,
                public_key.to_owned(),
                unsigned,
                network.to_owned(),
            )
            .unwrap();
        assert_eq!(signed, expected_signed);
    }

    #[test]
    fn native_sep53_message_paths_match_shared_vector() {
        let vector = message_signing_vector();
        let case = &vector["cases"][0];
        let secret = case["secret"].as_str().unwrap();
        let public_key = case["public_key"].as_str().unwrap();
        let message = decode_hex(case["message_hex"].as_str().unwrap());
        let expected_payload = decode_hex(case["encoded_message_hex"].as_str().unwrap());
        let expected_hash = decode_hex(case["message_hash_hex"].as_str().unwrap());
        let expected_signature = decode_hex(case["signature_hex"].as_str().unwrap());

        let api = FresnicaSdkApi::new();
        let protected = api
            .protect_secret(
                secret.to_owned(),
                "passcode".to_owned(),
                Some(public_key.to_owned()),
            )
            .unwrap();
        let unlock_key = api
            .derive_unlock_key(
                protected.envelope_json.clone(),
                "passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();

        let signature = api
            .sign_message(
                protected.envelope_json.clone(),
                unlock_key,
                public_key.to_owned(),
                message.clone(),
            )
            .unwrap();
        assert_eq!(signature, expected_signature);

        let passcode_signature = api
            .sign_message_with_passcode(
                protected.envelope_json,
                "passcode".to_owned(),
                public_key.to_owned(),
                message.clone(),
            )
            .unwrap();
        assert_eq!(passcode_signature, expected_signature);

        let request = api.prepare_message_signing(message.clone());
        assert_eq!(request.message, message);
        assert_eq!(request.encoded_message, expected_payload);
        assert_eq!(request.message_hash, expected_hash);
        api.verify_message_signature(
            request.message.clone(),
            public_key.to_owned(),
            expected_signature,
        )
        .unwrap();

        let invalid = api
            .verify_message_signature(request.message, public_key.to_owned(), vec![0u8; 64])
            .unwrap_err();
        assert_eq!(invalid.code(), NativeSdkErrorCode::InvalidMessageSignature);
    }

    #[test]
    fn boundary_errors_remain_stable() {
        let vector = signing_vector();
        let secret = vector["cases"][0]["secret"].as_str().unwrap();
        let api = FresnicaSdkApi::new();
        let mismatch = api
            .protect_secret(
                secret.to_owned(),
                "passcode".to_owned(),
                Some(OTHER_CLASSIC.to_owned()),
            )
            .err()
            .unwrap();
        assert_eq!(mismatch.code(), NativeSdkErrorCode::IdentityMismatch);
        assert_eq!(
            serde_json::to_value(&mismatch).unwrap()["code"],
            "identity-mismatch"
        );
    }

    #[test]
    fn external_ed25519_roundtrips_shared_vector() {
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

        let api = FresnicaSdkApi::new();
        let request = api
            .prepare_ed25519_signing(unsigned, network.to_owned())
            .unwrap();
        assert_eq!(request.transaction_hash, expected_hash);
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

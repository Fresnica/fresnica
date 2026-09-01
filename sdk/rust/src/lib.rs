//! Platform-neutral SDK contract over `fresnica_core::CoreClientApi`.
//!
//! This crate defines the stable semantic boundary shared by native SDK
//! packaging, future WASM bindings, and framework adapters. It deliberately
//! contains no UniFFI, React Native, platform secure-storage, networking, or UI
//! behavior. Cryptographic authority remains in `fresnica-core`.

use std::{error::Error, fmt};

use fresnica_core::{
    ClientAccountKind, ClientApiError, ClientApiErrorCode, ClientProtectedSoftwareSigner,
    CoreClientApi, ExportedSigningMaterial, WalletUnlockKey, CLIENT_API_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

/// Version of the platform-neutral Fresnica SDK semantic contract.
pub const SDK_API_VERSION: u64 = 4;

/// Stateless platform-neutral entry point.
///
/// The SDK intentionally carries no application session, persistence, network,
/// or OS-authentication state. Each operation delegates cryptographic behavior
/// to a short-lived `CoreClientApi` instance.
pub struct FresnicaSdk;

impl Default for FresnicaSdk {
    fn default() -> Self {
        Self::new()
    }
}

impl FresnicaSdk {
    pub fn new() -> Self {
        Self
    }

    fn core(&self) -> CoreClientApi {
        CoreClientApi::new()
    }

    pub fn version(&self) -> SdkVersion {
        SdkVersion {
            sdk_api_version: SDK_API_VERSION,
            core_client_api_version: CLIENT_API_VERSION,
        }
    }

    pub fn parse_account(&self, address: String) -> Result<SdkAccountIdentity, SdkError> {
        let identity = self
            .core()
            .parse_account(&address)
            .map_err(SdkError::from)?;
        Ok(SdkAccountIdentity {
            kind: match identity.kind {
                ClientAccountKind::Classic => SdkAccountKind::Classic,
                ClientAccountKind::Contract => SdkAccountKind::Contract,
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
    ) -> Result<SdkProtectedSoftwareSigner, SdkError> {
        let secret = Zeroizing::new(secret);
        let passcode = Zeroizing::new(passcode);
        let protected = self
            .core()
            .protect_secret(
                secret.as_str(),
                passcode.as_str(),
                expected_signer_public_key.as_deref(),
            )
            .map_err(SdkError::from)?;
        sdk_protected_signer(protected)
    }

    pub fn protect_mnemonic(
        &self,
        mnemonic: String,
        mnemonic_passphrase: String,
        index: u32,
        language: Option<String>,
        passcode: String,
        expected_signer_public_key: Option<String>,
    ) -> Result<SdkProtectedSoftwareSigner, SdkError> {
        let mnemonic = Zeroizing::new(mnemonic);
        let mnemonic_passphrase = Zeroizing::new(mnemonic_passphrase);
        let passcode = Zeroizing::new(passcode);
        let protected = self
            .core()
            .protect_mnemonic(
                mnemonic.as_str(),
                mnemonic_passphrase.as_str(),
                sdk_usize(index, "index")?,
                language.as_deref(),
                passcode.as_str(),
                expected_signer_public_key.as_deref(),
            )
            .map_err(SdkError::from)?;
        sdk_protected_signer(protected)
    }

    pub fn generate_mnemonic(
        &self,
        language: String,
        strength: u32,
        mnemonic_passphrase: String,
        index: u32,
        passcode: String,
    ) -> Result<SdkGeneratedMnemonic, SdkError> {
        let mnemonic_passphrase = Zeroizing::new(mnemonic_passphrase);
        let passcode = Zeroizing::new(passcode);
        let generated = self
            .core()
            .generate_mnemonic(
                &language,
                sdk_usize(strength, "strength")?,
                mnemonic_passphrase.as_str(),
                sdk_usize(index, "index")?,
                passcode.as_str(),
            )
            .map_err(SdkError::from)?;
        Ok(SdkGeneratedMnemonic {
            signer: sdk_protected_signer(generated.signer)?,
            mnemonic: generated.mnemonic.as_str().to_owned(),
            language: generated.language,
            index: sdk_u32(generated.index, "index")?,
        })
    }

    pub fn derive_mnemonic_signer(
        &self,
        source_envelope_json: String,
        passcode: String,
        expected_source_signer_public_key: String,
        index: u32,
    ) -> Result<SdkProtectedSoftwareSigner, SdkError> {
        let envelope = parse_envelope(&source_envelope_json)?;
        let passcode = Zeroizing::new(passcode);
        let protected = self
            .core()
            .derive_mnemonic_signer(
                &envelope,
                passcode.as_str(),
                &expected_source_signer_public_key,
                sdk_usize(index, "index")?,
            )
            .map_err(SdkError::from)?;
        sdk_protected_signer(protected)
    }

    pub fn reprotect(
        &self,
        envelope_json: String,
        current_passcode: String,
        new_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<SdkProtectedSoftwareSigner, SdkError> {
        let envelope = parse_envelope(&envelope_json)?;
        let current_passcode = Zeroizing::new(current_passcode);
        let new_passcode = Zeroizing::new(new_passcode);
        let protected = self
            .core()
            .reprotect(
                &envelope,
                current_passcode.as_str(),
                new_passcode.as_str(),
                &expected_signer_public_key,
            )
            .map_err(SdkError::from)?;
        sdk_protected_signer(protected)
    }

    /// Derive a verified 32-byte unlock key for native routine signing.
    ///
    /// Framework adapters must not expose this material to JavaScript/Dart merely
    /// for convenience. Browser/WASM authorization requires a separate reviewed
    /// security design.
    pub fn derive_unlock_key(
        &self,
        envelope_json: String,
        passcode: String,
        expected_signer_public_key: String,
    ) -> Result<Vec<u8>, SdkError> {
        let envelope = parse_envelope(&envelope_json)?;
        let passcode = Zeroizing::new(passcode);
        let key = self
            .core()
            .derive_unlock_key(&envelope, passcode.as_str(), &expected_signer_public_key)
            .map_err(SdkError::from)?;
        Ok(key.as_bytes().to_vec())
    }

    pub fn validate_unlock_key(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
    ) -> Result<(), SdkError> {
        let envelope = parse_envelope(&envelope_json)?;
        let unlock_key = sdk_unlock_key(unlock_key)?;
        self.core()
            .validate_unlock_key(&envelope, &unlock_key, &expected_signer_public_key)
            .map_err(SdkError::from)
    }

    pub fn sign_transaction_xdr(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<Vec<u8>, SdkError> {
        let envelope = parse_envelope(&envelope_json)?;
        let unlock_key = sdk_unlock_key(unlock_key)?;
        self.core()
            .sign_transaction_xdr(
                &envelope,
                &unlock_key,
                &expected_signer_public_key,
                &transaction_xdr,
                &network_passphrase,
            )
            .map_err(SdkError::from)
    }

    /// Sign with a fresh application passcode without exposing `WalletUnlockKey`
    /// material outside Rust. This is the routine signing path for environments
    /// such as browsers that cannot provide a reviewed native secure-key store.
    pub fn sign_transaction_xdr_with_passcode(
        &self,
        envelope_json: String,
        passcode: String,
        expected_signer_public_key: String,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<Vec<u8>, SdkError> {
        let envelope = parse_envelope(&envelope_json)?;
        let passcode = Zeroizing::new(passcode);
        let core = self.core();
        let unlock_key = core
            .derive_unlock_key(&envelope, passcode.as_str(), &expected_signer_public_key)
            .map_err(SdkError::from)?;
        core.sign_transaction_xdr(
            &envelope,
            &unlock_key,
            &expected_signer_public_key,
            &transaction_xdr,
            &network_passphrase,
        )
        .map_err(SdkError::from)
    }

    pub fn sign_soroban_authorization_xdr(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
        authorization_entry_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<Vec<u8>, SdkError> {
        let envelope = parse_envelope(&envelope_json)?;
        let unlock_key = sdk_unlock_key(unlock_key)?;
        self.core()
            .sign_soroban_authorization_xdr(
                &envelope,
                &unlock_key,
                &expected_signer_public_key,
                &authorization_entry_xdr,
                &network_passphrase,
            )
            .map_err(SdkError::from)
    }

    /// Sign a Soroban authorization entry with a fresh application passcode
    /// without exposing `WalletUnlockKey` material outside Rust.
    pub fn sign_soroban_authorization_xdr_with_passcode(
        &self,
        envelope_json: String,
        passcode: String,
        expected_signer_public_key: String,
        authorization_entry_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<Vec<u8>, SdkError> {
        let envelope = parse_envelope(&envelope_json)?;
        let passcode = Zeroizing::new(passcode);
        let core = self.core();
        let unlock_key = core
            .derive_unlock_key(&envelope, passcode.as_str(), &expected_signer_public_key)
            .map_err(SdkError::from)?;
        core.sign_soroban_authorization_xdr(
            &envelope,
            &unlock_key,
            &expected_signer_public_key,
            &authorization_entry_xdr,
            &network_passphrase,
        )
        .map_err(SdkError::from)
    }

    /// Explicitly declassify recovery material using a fresh application passcode.
    pub fn reveal(
        &self,
        envelope_json: String,
        fresh_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<SdkExportedSigningMaterial, SdkError> {
        let envelope = parse_envelope(&envelope_json)?;
        let fresh_passcode = Zeroizing::new(fresh_passcode);
        let material = self
            .core()
            .reveal(
                &envelope,
                fresh_passcode.as_str(),
                &expected_signer_public_key,
            )
            .map_err(SdkError::from)?;
        match material {
            ExportedSigningMaterial::Secret { secret } => Ok(SdkExportedSigningMaterial {
                kind: SdkSigningMaterialKind::Secret,
                secret: Some(secret.as_str().to_owned()),
                mnemonic: None,
                mnemonic_passphrase: None,
                index: None,
                language: None,
            }),
            ExportedSigningMaterial::Mnemonic {
                mnemonic,
                mnemonic_passphrase,
                index,
                language,
            } => Ok(SdkExportedSigningMaterial {
                kind: SdkSigningMaterialKind::Mnemonic,
                secret: None,
                mnemonic: Some(mnemonic.as_str().to_owned()),
                mnemonic_passphrase: Some(mnemonic_passphrase.as_str().to_owned()),
                index: Some(sdk_u32(index, "index")?),
                language: Some(language),
            }),
        }
    }

    pub fn prepare_ed25519_signing(
        &self,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<SdkEd25519SigningRequest, SdkError> {
        let request = self
            .core()
            .prepare_ed25519_signing(&transaction_xdr, &network_passphrase)
            .map_err(SdkError::from)?;
        Ok(SdkEd25519SigningRequest {
            transaction_hash: request.transaction_hash.to_vec(),
            transaction_xdr: request.transaction_xdr,
            network_passphrase: request.network_passphrase,
        })
    }

    pub fn prepare_soroban_authorization_signing(
        &self,
        authorization_entry_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<SdkSorobanAuthorizationSigningRequest, SdkError> {
        let request = self
            .core()
            .prepare_soroban_authorization_signing(&authorization_entry_xdr, &network_passphrase)
            .map_err(SdkError::from)?;
        Ok(SdkSorobanAuthorizationSigningRequest {
            authorization_hash: request.authorization_hash.to_vec(),
            authorization_entry_xdr: request.authorization_entry_xdr,
            authorization_preimage_xdr: request.authorization_preimage_xdr,
            network_passphrase: request.network_passphrase,
        })
    }

    pub fn apply_ed25519_signature(
        &self,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
        signer_public_key: String,
        signature: Vec<u8>,
    ) -> Result<Vec<u8>, SdkError> {
        self.core()
            .apply_ed25519_signature(
                &transaction_xdr,
                &network_passphrase,
                &signer_public_key,
                &signature,
            )
            .map_err(SdkError::from)
    }

    pub fn apply_soroban_ed25519_signature(
        &self,
        authorization_entry_xdr: Vec<u8>,
        network_passphrase: String,
        signer_public_key: String,
        signature: Vec<u8>,
    ) -> Result<Vec<u8>, SdkError> {
        self.core()
            .apply_soroban_ed25519_signature(
                &authorization_entry_xdr,
                &network_passphrase,
                &signer_public_key,
                &signature,
            )
            .map_err(SdkError::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkVersion {
    pub sdk_api_version: u64,
    pub core_client_api_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SdkAccountKind {
    Classic,
    Contract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkAccountIdentity {
    pub kind: SdkAccountKind,
    pub address: String,
    pub public_key: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkProtectedSoftwareSigner {
    pub signer_public_key: String,
    pub envelope_json: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkGeneratedMnemonic {
    pub signer: SdkProtectedSoftwareSigner,
    pub mnemonic: String,
    pub language: String,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkEd25519SigningRequest {
    pub transaction_hash: Vec<u8>,
    pub transaction_xdr: Vec<u8>,
    pub network_passphrase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkSorobanAuthorizationSigningRequest {
    pub authorization_hash: Vec<u8>,
    pub authorization_entry_xdr: Vec<u8>,
    pub authorization_preimage_xdr: Vec<u8>,
    pub network_passphrase: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SdkSigningMaterialKind {
    Secret,
    Mnemonic,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkExportedSigningMaterial {
    pub kind: SdkSigningMaterialKind,
    pub secret: Option<String>,
    pub mnemonic: Option<String>,
    pub mnemonic_passphrase: Option<String>,
    pub index: Option<u32>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SdkErrorCode {
    InvalidInput,
    InvalidPasscode,
    InvalidUnlockKey,
    InvalidProtectedData,
    IdentityMismatch,
    InvalidTransaction,
    InvalidAuthorization,
    CoreError,
}

impl SdkErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid-input",
            Self::InvalidPasscode => "invalid-passcode",
            Self::InvalidUnlockKey => "invalid-unlock-key",
            Self::InvalidProtectedData => "invalid-protected-data",
            Self::IdentityMismatch => "identity-mismatch",
            Self::InvalidTransaction => "invalid-transaction",
            Self::InvalidAuthorization => "invalid-authorization",
            Self::CoreError => "core-error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkError {
    pub code: SdkErrorCode,
    pub message: String,
}

impl SdkError {
    fn new(code: SdkErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SdkError {}

impl From<ClientApiError> for SdkError {
    fn from(error: ClientApiError) -> Self {
        let code = match error.code() {
            ClientApiErrorCode::InvalidInput => SdkErrorCode::InvalidInput,
            ClientApiErrorCode::InvalidPasscode => SdkErrorCode::InvalidPasscode,
            ClientApiErrorCode::InvalidUnlockKey => SdkErrorCode::InvalidUnlockKey,
            ClientApiErrorCode::InvalidProtectedData => SdkErrorCode::InvalidProtectedData,
            ClientApiErrorCode::IdentityMismatch => SdkErrorCode::IdentityMismatch,
            ClientApiErrorCode::InvalidTransaction => SdkErrorCode::InvalidTransaction,
            ClientApiErrorCode::InvalidAuthorization => SdkErrorCode::InvalidAuthorization,
            ClientApiErrorCode::CoreError => SdkErrorCode::CoreError,
            _ => SdkErrorCode::CoreError,
        };
        Self::new(code, error.message())
    }
}

fn sdk_protected_signer(
    protected: ClientProtectedSoftwareSigner,
) -> Result<SdkProtectedSoftwareSigner, SdkError> {
    let envelope_json = serde_json::to_string(&protected.envelope).map_err(|error| {
        SdkError::new(
            SdkErrorCode::CoreError,
            format!("unable to serialize protected signer envelope: {error}"),
        )
    })?;
    Ok(SdkProtectedSoftwareSigner {
        signer_public_key: protected.signer_public_key,
        envelope_json,
    })
}

fn parse_envelope(envelope_json: &str) -> Result<Value, SdkError> {
    serde_json::from_str(envelope_json).map_err(|_| {
        SdkError::new(
            SdkErrorCode::InvalidProtectedData,
            "protected signer envelope is not valid JSON",
        )
    })
}

fn sdk_unlock_key(bytes: Vec<u8>) -> Result<WalletUnlockKey, SdkError> {
    let bytes = Zeroizing::new(bytes);
    if bytes.len() != 32 {
        return Err(SdkError::new(
            SdkErrorCode::InvalidUnlockKey,
            "wallet unlock key must be exactly 32 bytes",
        ));
    }
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(bytes.as_slice());
    let key = WalletUnlockKey::from_bytes(key_bytes);
    key_bytes.zeroize();
    Ok(key)
}

fn sdk_usize(value: u32, field: &'static str) -> Result<usize, SdkError> {
    usize::try_from(value).map_err(|_| {
        SdkError::new(
            SdkErrorCode::InvalidInput,
            format!("{field} is outside the supported range"),
        )
    })
}

fn sdk_u32(value: usize, field: &'static str) -> Result<u32, SdkError> {
    u32::try_from(value).map_err(|_| {
        SdkError::new(
            SdkErrorCode::InvalidProtectedData,
            format!("{field} is outside the SDK range"),
        )
    })
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

    fn soroban_authorization_vector() -> Value {
        serde_json::from_str(include_str!(
            "../../../spec/test-vectors/soroban-authorization-signing-v1.json"
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
    fn sdk_is_safe_for_native_threading() {
        assert_send_sync::<FresnicaSdk>();
    }

    #[test]
    fn versions_and_account_identity_are_stable() {
        let sdk = FresnicaSdk::new();
        let version = sdk.version();
        assert_eq!(version.sdk_api_version, SDK_API_VERSION);
        assert_eq!(version.core_client_api_version, CLIENT_API_VERSION);

        let classic = sdk.parse_account(CLASSIC.to_owned()).unwrap();
        assert_eq!(classic.kind, SdkAccountKind::Classic);
        assert_eq!(classic.address, CLASSIC);
        assert_eq!(classic.public_key.as_deref(), Some(CLASSIC));

        let contract = sdk.parse_account(CONTRACT.to_owned()).unwrap();
        assert_eq!(contract.kind, SdkAccountKind::Contract);
        assert_eq!(contract.address, CONTRACT);
        assert_eq!(contract.public_key, None);
    }

    #[test]
    fn protected_software_signer_roundtrips_shared_transaction_vector() {
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

        let sdk = FresnicaSdk::new();
        let protected = sdk
            .protect_secret(
                secret.to_owned(),
                "old-passcode".to_owned(),
                Some(public_key.to_owned()),
            )
            .unwrap();
        assert_eq!(protected.signer_public_key, public_key);
        assert!(!protected.envelope_json.contains(secret));

        let unlock_key = sdk
            .derive_unlock_key(
                protected.envelope_json.clone(),
                "old-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        assert_eq!(unlock_key.len(), 32);
        sdk.validate_unlock_key(
            protected.envelope_json.clone(),
            unlock_key.clone(),
            public_key.to_owned(),
        )
        .unwrap();

        let signed = sdk
            .sign_transaction_xdr(
                protected.envelope_json.clone(),
                unlock_key.clone(),
                public_key.to_owned(),
                unsigned.clone(),
                network.to_owned(),
            )
            .unwrap();
        assert_eq!(signed, expected_signed);

        let passcode_signed = sdk
            .sign_transaction_xdr_with_passcode(
                protected.envelope_json.clone(),
                "old-passcode".to_owned(),
                public_key.to_owned(),
                unsigned,
                network.to_owned(),
            )
            .unwrap();
        assert_eq!(passcode_signed, expected_signed);

        let passcode_error = sdk
            .sign_transaction_xdr_with_passcode(
                protected.envelope_json.clone(),
                "wrong-passcode".to_owned(),
                public_key.to_owned(),
                Vec::new(),
                network.to_owned(),
            )
            .unwrap_err();
        assert_eq!(passcode_error.code, SdkErrorCode::InvalidPasscode);

        let revealed = sdk
            .reveal(
                protected.envelope_json.clone(),
                "old-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        assert_eq!(revealed.kind, SdkSigningMaterialKind::Secret);
        assert_eq!(revealed.secret.as_deref(), Some(secret));

        let reprotected = sdk
            .reprotect(
                protected.envelope_json,
                "old-passcode".to_owned(),
                "new-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        let old_error = sdk
            .derive_unlock_key(
                reprotected.envelope_json.clone(),
                "old-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap_err();
        assert_eq!(old_error.code, SdkErrorCode::InvalidPasscode);

        let new_key = sdk
            .derive_unlock_key(
                reprotected.envelope_json,
                "new-passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();
        assert_ne!(new_key, unlock_key);
    }

    #[test]
    fn derives_mnemonic_signer_from_existing_protected_source() {
        let sdk = FresnicaSdk::new();
        let generated = sdk
            .generate_mnemonic(
                "english".to_owned(),
                128,
                String::new(),
                0,
                "passcode".to_owned(),
            )
            .unwrap();
        let source_public_key = generated.signer.signer_public_key.clone();
        let derived = sdk
            .derive_mnemonic_signer(
                generated.signer.envelope_json,
                "passcode".to_owned(),
                source_public_key.clone(),
                1,
            )
            .unwrap();
        assert_ne!(derived.signer_public_key, source_public_key);

        let revealed = sdk
            .reveal(
                derived.envelope_json,
                "passcode".to_owned(),
                derived.signer_public_key,
            )
            .unwrap();
        assert_eq!(revealed.kind, SdkSigningMaterialKind::Mnemonic);
        assert_eq!(revealed.index, Some(1));
    }

    #[test]
    fn mnemonic_import_generation_and_reveal_share_the_same_contract() {
        let sdk = FresnicaSdk::new();
        let imported = sdk
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

        let generated = sdk
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

        let revealed = sdk
            .reveal(
                generated.signer.envelope_json,
                "passcode".to_owned(),
                generated.signer.signer_public_key,
            )
            .unwrap();
        assert_eq!(revealed.kind, SdkSigningMaterialKind::Mnemonic);
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
        let sdk = FresnicaSdk::new();

        let mismatch = sdk
            .protect_secret(
                secret.to_owned(),
                "passcode".to_owned(),
                Some(OTHER_CLASSIC.to_owned()),
            )
            .err()
            .unwrap();
        assert_eq!(mismatch.code, SdkErrorCode::IdentityMismatch);
        assert_eq!(
            serde_json::to_value(&mismatch).unwrap()["code"],
            "identity-mismatch"
        );

        let malformed = sdk
            .derive_unlock_key(
                "not-json".to_owned(),
                "passcode".to_owned(),
                CLASSIC.to_owned(),
            )
            .unwrap_err();
        assert_eq!(malformed.code, SdkErrorCode::InvalidProtectedData);

        let protected = sdk
            .protect_secret(secret.to_owned(), "passcode".to_owned(), None)
            .unwrap();
        let bad_key = sdk
            .validate_unlock_key(protected.envelope_json, vec![0u8; 31], CLASSIC.to_owned())
            .unwrap_err();
        assert_eq!(bad_key.code, SdkErrorCode::InvalidUnlockKey);
    }

    #[test]
    fn external_ed25519_signing_roundtrips_shared_vector() {
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

        let sdk = FresnicaSdk::new();
        let request = sdk
            .prepare_ed25519_signing(unsigned.clone(), network.to_owned())
            .unwrap();
        assert_eq!(request.transaction_hash, expected_hash);
        assert_eq!(request.transaction_xdr, unsigned);
        assert_eq!(request.network_passphrase, network);

        let signed = sdk
            .apply_ed25519_signature(
                request.transaction_xdr,
                network.to_owned(),
                public_key.to_owned(),
                signature,
            )
            .unwrap();
        assert_eq!(signed, expected_signed);
    }

    #[test]
    fn protected_software_signer_roundtrips_shared_soroban_authorization_vector() {
        let vector = soroban_authorization_vector();
        let case = &vector["cases"][0];
        let secret = case["secret"].as_str().unwrap();
        let public_key = case["public_key"].as_str().unwrap();
        let network = case["network_passphrase"].as_str().unwrap();
        let unsigned = STANDARD
            .decode(case["unsigned_entry_xdr_base64"].as_str().unwrap())
            .unwrap();
        let expected_signed = STANDARD
            .decode(case["signed_entry_xdr_base64"].as_str().unwrap())
            .unwrap();

        let sdk = FresnicaSdk::new();
        let protected = sdk
            .protect_secret(
                secret.to_owned(),
                "passcode".to_owned(),
                Some(public_key.to_owned()),
            )
            .unwrap();
        let unlock_key = sdk
            .derive_unlock_key(
                protected.envelope_json.clone(),
                "passcode".to_owned(),
                public_key.to_owned(),
            )
            .unwrap();

        let signed = sdk
            .sign_soroban_authorization_xdr(
                protected.envelope_json.clone(),
                unlock_key,
                public_key.to_owned(),
                unsigned.clone(),
                network.to_owned(),
            )
            .unwrap();
        assert_eq!(signed, expected_signed);

        let passcode_signed = sdk
            .sign_soroban_authorization_xdr_with_passcode(
                protected.envelope_json.clone(),
                "passcode".to_owned(),
                public_key.to_owned(),
                unsigned,
                network.to_owned(),
            )
            .unwrap();
        assert_eq!(passcode_signed, expected_signed);

        let error = sdk
            .sign_soroban_authorization_xdr_with_passcode(
                protected.envelope_json,
                "wrong-passcode".to_owned(),
                public_key.to_owned(),
                Vec::new(),
                network.to_owned(),
            )
            .unwrap_err();
        assert_eq!(error.code, SdkErrorCode::InvalidPasscode);
    }

    #[test]
    fn external_ed25519_soroban_authorization_roundtrips_shared_vector() {
        let vector = soroban_authorization_vector();
        let case = &vector["cases"][0];
        let public_key = case["public_key"].as_str().unwrap();
        let network = case["network_passphrase"].as_str().unwrap();
        let unsigned = STANDARD
            .decode(case["unsigned_entry_xdr_base64"].as_str().unwrap())
            .unwrap();
        let expected_preimage = STANDARD
            .decode(case["authorization_preimage_xdr_base64"].as_str().unwrap())
            .unwrap();
        let expected_hash = decode_hex(case["authorization_hash_hex"].as_str().unwrap());
        let signature = decode_hex(case["signature_hex"].as_str().unwrap());
        let expected_signed = STANDARD
            .decode(case["signed_entry_xdr_base64"].as_str().unwrap())
            .unwrap();

        let sdk = FresnicaSdk::new();
        let request = sdk
            .prepare_soroban_authorization_signing(unsigned.clone(), network.to_owned())
            .unwrap();
        assert_eq!(request.authorization_hash, expected_hash);
        assert_eq!(request.authorization_entry_xdr, unsigned);
        assert_eq!(request.authorization_preimage_xdr, expected_preimage);
        assert_eq!(request.network_passphrase, network);

        let signed = sdk
            .apply_soroban_ed25519_signature(
                request.authorization_entry_xdr,
                network.to_owned(),
                public_key.to_owned(),
                signature,
            )
            .unwrap();
        assert_eq!(signed, expected_signed);
    }

    #[test]
    fn soroban_authorization_errors_keep_a_distinct_stable_code() {
        let sdk = FresnicaSdk::new();
        let malformed = sdk
            .prepare_soroban_authorization_signing(
                b"not-xdr".to_vec(),
                "Test SDF Network ; September 2015".to_owned(),
            )
            .unwrap_err();
        assert_eq!(malformed.code, SdkErrorCode::InvalidAuthorization);
        assert_eq!(
            serde_json::to_value(&malformed).unwrap()["code"],
            "invalid-authorization"
        );

        let vector = soroban_authorization_vector();
        let case = &vector["cases"][0];
        let unsigned = STANDARD
            .decode(case["unsigned_entry_xdr_base64"].as_str().unwrap())
            .unwrap();
        let invalid_signature = vec![0u8; 64];
        let error = sdk
            .apply_soroban_ed25519_signature(
                unsigned,
                case["network_passphrase"].as_str().unwrap().to_owned(),
                case["public_key"].as_str().unwrap().to_owned(),
                invalid_signature,
            )
            .unwrap_err();
        assert_eq!(error.code, SdkErrorCode::InvalidAuthorization);
    }
}

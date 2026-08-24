//! FFI-neutral mobile facade over `fresnica_core::CoreClientApi`.
//!
//! This crate is intentionally not a UniFFI, JNI, Swift, or React Native binding.
//! It freezes the mobile-facing data shapes first so platform adapters can stay
//! thin and cannot accidentally reimplement Core cryptography or identity rules.

use std::{error::Error, fmt};

use fresnica_core::{
    ClientAccountKind, ClientApiError, ClientApiErrorCode, ClientProtectedSoftwareSigner,
    CoreClientApi, ExportedSigningMaterial, WalletUnlockKey, CLIENT_API_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

pub const MOBILE_BINDING_API_VERSION: u64 = 1;

pub struct MobileCoreApi {
    core: CoreClientApi,
}

impl Default for MobileCoreApi {
    fn default() -> Self {
        Self::new()
    }
}

impl MobileCoreApi {
    pub fn new() -> Self {
        Self {
            core: CoreClientApi::new(),
        }
    }

    pub fn version(&self) -> MobileCoreVersion {
        MobileCoreVersion {
            mobile_binding_api_version: MOBILE_BINDING_API_VERSION,
            core_client_api_version: CLIENT_API_VERSION,
        }
    }

    pub fn parse_account(&self, address: String) -> Result<MobileAccountIdentity, MobileCoreError> {
        let identity = self.core.parse_account(&address).map_err(MobileCoreError::from)?;
        Ok(MobileAccountIdentity {
            kind: match identity.kind {
                ClientAccountKind::Classic => MobileAccountKind::Classic,
                ClientAccountKind::Contract => MobileAccountKind::Contract,
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
        let secret = Zeroizing::new(secret);
        let passcode = Zeroizing::new(passcode);
        let protected = self
            .core
            .protect_secret(
                secret.as_str(),
                passcode.as_str(),
                expected_signer_public_key.as_deref(),
            )
            .map_err(MobileCoreError::from)?;
        mobile_protected_signer(protected)
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
        let mnemonic = Zeroizing::new(mnemonic);
        let mnemonic_passphrase = Zeroizing::new(mnemonic_passphrase);
        let passcode = Zeroizing::new(passcode);
        let protected = self
            .core
            .protect_mnemonic(
                mnemonic.as_str(),
                mnemonic_passphrase.as_str(),
                binding_usize(index, "index")?,
                language.as_deref(),
                passcode.as_str(),
                expected_signer_public_key.as_deref(),
            )
            .map_err(MobileCoreError::from)?;
        mobile_protected_signer(protected)
    }

    pub fn generate_mnemonic(
        &self,
        language: String,
        strength: u32,
        mnemonic_passphrase: String,
        index: u32,
        passcode: String,
    ) -> Result<MobileGeneratedMnemonic, MobileCoreError> {
        let mnemonic_passphrase = Zeroizing::new(mnemonic_passphrase);
        let passcode = Zeroizing::new(passcode);
        let generated = self
            .core
            .generate_mnemonic(
                &language,
                binding_usize(strength, "strength")?,
                mnemonic_passphrase.as_str(),
                binding_usize(index, "index")?,
                passcode.as_str(),
            )
            .map_err(MobileCoreError::from)?;
        Ok(MobileGeneratedMnemonic {
            signer: mobile_protected_signer(generated.signer)?,
            mnemonic: generated.mnemonic.as_str().to_owned(),
            language: generated.language,
            index: binding_u32(generated.index, "index")?,
        })
    }

    pub fn reprotect(
        &self,
        envelope_json: String,
        current_passcode: String,
        new_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<MobileProtectedSoftwareSigner, MobileCoreError> {
        let envelope = parse_envelope(&envelope_json)?;
        let current_passcode = Zeroizing::new(current_passcode);
        let new_passcode = Zeroizing::new(new_passcode);
        let protected = self
            .core
            .reprotect(
                &envelope,
                current_passcode.as_str(),
                new_passcode.as_str(),
                &expected_signer_public_key,
            )
            .map_err(MobileCoreError::from)?;
        mobile_protected_signer(protected)
    }

    pub fn derive_unlock_key(
        &self,
        envelope_json: String,
        passcode: String,
        expected_signer_public_key: String,
    ) -> Result<Vec<u8>, MobileCoreError> {
        let envelope = parse_envelope(&envelope_json)?;
        let passcode = Zeroizing::new(passcode);
        let key = self
            .core
            .derive_unlock_key(&envelope, passcode.as_str(), &expected_signer_public_key)
            .map_err(MobileCoreError::from)?;
        Ok(key.as_bytes().to_vec())
    }

    pub fn validate_unlock_key(
        &self,
        envelope_json: String,
        unlock_key: Vec<u8>,
        expected_signer_public_key: String,
    ) -> Result<(), MobileCoreError> {
        let envelope = parse_envelope(&envelope_json)?;
        let unlock_key = binding_unlock_key(unlock_key)?;
        self.core
            .validate_unlock_key(&envelope, &unlock_key, &expected_signer_public_key)
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
        let envelope = parse_envelope(&envelope_json)?;
        let unlock_key = binding_unlock_key(unlock_key)?;
        self.core
            .sign_transaction_xdr(
                &envelope,
                &unlock_key,
                &expected_signer_public_key,
                &transaction_xdr,
                &network_passphrase,
            )
            .map_err(MobileCoreError::from)
    }

    pub fn reveal(
        &self,
        envelope_json: String,
        fresh_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<MobileExportedSigningMaterial, MobileCoreError> {
        let envelope = parse_envelope(&envelope_json)?;
        let fresh_passcode = Zeroizing::new(fresh_passcode);
        let material = self
            .core
            .reveal(
                &envelope,
                fresh_passcode.as_str(),
                &expected_signer_public_key,
            )
            .map_err(MobileCoreError::from)?;
        match material {
            ExportedSigningMaterial::Secret { secret } => Ok(MobileExportedSigningMaterial {
                kind: MobileSigningMaterialKind::Secret,
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
            } => Ok(MobileExportedSigningMaterial {
                kind: MobileSigningMaterialKind::Mnemonic,
                secret: None,
                mnemonic: Some(mnemonic.as_str().to_owned()),
                mnemonic_passphrase: Some(mnemonic_passphrase.as_str().to_owned()),
                index: Some(binding_u32(index, "index")?),
                language: Some(language),
            }),
        }
    }

    pub fn prepare_ed25519_signing(
        &self,
        transaction_xdr: Vec<u8>,
        network_passphrase: String,
    ) -> Result<MobileEd25519SigningRequest, MobileCoreError> {
        let request = self
            .core
            .prepare_ed25519_signing(&transaction_xdr, &network_passphrase)
            .map_err(MobileCoreError::from)?;
        Ok(MobileEd25519SigningRequest {
            transaction_hash: request.transaction_hash.to_vec(),
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
        self.core
            .apply_ed25519_signature(
                &transaction_xdr,
                &network_passphrase,
                &signer_public_key,
                &signature,
            )
            .map_err(MobileCoreError::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileCoreVersion {
    pub mobile_binding_api_version: u64,
    pub core_client_api_version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MobileAccountKind {
    Classic,
    Contract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileAccountIdentity {
    pub kind: MobileAccountKind,
    pub address: String,
    pub public_key: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileProtectedSoftwareSigner {
    pub signer_public_key: String,
    pub envelope_json: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileGeneratedMnemonic {
    pub signer: MobileProtectedSoftwareSigner,
    pub mnemonic: String,
    pub language: String,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileEd25519SigningRequest {
    pub transaction_hash: Vec<u8>,
    pub transaction_xdr: Vec<u8>,
    pub network_passphrase: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MobileSigningMaterialKind {
    Secret,
    Mnemonic,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileExportedSigningMaterial {
    pub kind: MobileSigningMaterialKind,
    pub secret: Option<String>,
    pub mnemonic: Option<String>,
    pub mnemonic_passphrase: Option<String>,
    pub index: Option<u32>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileCoreError {
    pub code: MobileCoreErrorCode,
    pub message: String,
}

impl MobileCoreError {
    fn new(code: MobileCoreErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for MobileCoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for MobileCoreError {}

impl From<ClientApiError> for MobileCoreError {
    fn from(error: ClientApiError) -> Self {
        let code = match error.code() {
            ClientApiErrorCode::InvalidInput => MobileCoreErrorCode::InvalidInput,
            ClientApiErrorCode::InvalidPasscode => MobileCoreErrorCode::InvalidPasscode,
            ClientApiErrorCode::InvalidUnlockKey => MobileCoreErrorCode::InvalidUnlockKey,
            ClientApiErrorCode::InvalidProtectedData => MobileCoreErrorCode::InvalidProtectedData,
            ClientApiErrorCode::IdentityMismatch => MobileCoreErrorCode::IdentityMismatch,
            ClientApiErrorCode::InvalidTransaction => MobileCoreErrorCode::InvalidTransaction,
            ClientApiErrorCode::CoreError => MobileCoreErrorCode::CoreError,
            _ => MobileCoreErrorCode::CoreError,
        };
        Self::new(code, error.message())
    }
}

fn mobile_protected_signer(
    protected: ClientProtectedSoftwareSigner,
) -> Result<MobileProtectedSoftwareSigner, MobileCoreError> {
    let envelope_json = serde_json::to_string(&protected.envelope).map_err(|error| {
        MobileCoreError::new(
            MobileCoreErrorCode::CoreError,
            format!("unable to serialize protected signer envelope: {error}"),
        )
    })?;
    Ok(MobileProtectedSoftwareSigner {
        signer_public_key: protected.signer_public_key,
        envelope_json,
    })
}

fn parse_envelope(envelope_json: &str) -> Result<Value, MobileCoreError> {
    serde_json::from_str(envelope_json).map_err(|_| {
        MobileCoreError::new(
            MobileCoreErrorCode::InvalidProtectedData,
            "protected signer envelope is not valid JSON",
        )
    })
}

fn binding_unlock_key(bytes: Vec<u8>) -> Result<WalletUnlockKey, MobileCoreError> {
    let bytes = Zeroizing::new(bytes);
    if bytes.len() != 32 {
        return Err(MobileCoreError::new(
            MobileCoreErrorCode::InvalidUnlockKey,
            "wallet unlock key must be exactly 32 bytes",
        ));
    }
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(bytes.as_slice());
    let key = WalletUnlockKey::from_bytes(key_bytes);
    key_bytes.zeroize();
    Ok(key)
}

fn binding_usize(value: u32, field: &'static str) -> Result<usize, MobileCoreError> {
    usize::try_from(value).map_err(|_| {
        MobileCoreError::new(
            MobileCoreErrorCode::InvalidInput,
            format!("{field} is outside the supported range"),
        )
    })
}

fn binding_u32(value: usize, field: &'static str) -> Result<u32, MobileCoreError> {
    u32::try_from(value).map_err(|_| {
        MobileCoreError::new(
            MobileCoreErrorCode::InvalidProtectedData,
            format!("{field} is outside the mobile binding range"),
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
    const MNEMONIC: &str = "illness spike retreat truth genius clock brain pass fit cave bargain toe";
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

    #[test]
    fn versions_and_account_identity_are_stable() {
        let api = MobileCoreApi::new();
        let version = api.version();
        assert_eq!(version.mobile_binding_api_version, MOBILE_BINDING_API_VERSION);
        assert_eq!(version.core_client_api_version, CLIENT_API_VERSION);

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
        assert_eq!(old_error.code, MobileCoreErrorCode::InvalidPasscode);

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
        assert_eq!(revealed.mnemonic.as_deref(), Some(generated.mnemonic.as_str()));
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
            .unwrap_err();
        assert_eq!(mismatch.code, MobileCoreErrorCode::IdentityMismatch);

        let malformed = api
            .derive_unlock_key(
                "not-json".to_owned(),
                "passcode".to_owned(),
                CLASSIC.to_owned(),
            )
            .unwrap_err();
        assert_eq!(malformed.code, MobileCoreErrorCode::InvalidProtectedData);

        let protected = api
            .protect_secret(secret.to_owned(), "passcode".to_owned(), None)
            .unwrap();
        let bad_key = api
            .validate_unlock_key(
                protected.envelope_json,
                vec![0u8; 31],
                CLASSIC.to_owned(),
            )
            .unwrap_err();
        assert_eq!(bad_key.code, MobileCoreErrorCode::InvalidUnlockKey);
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

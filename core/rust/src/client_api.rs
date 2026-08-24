use serde_json::Value;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    derive_verified_unlock_key, export_signing_material, generate_protected_mnemonic,
    parse_transaction_envelope_xdr, protect_mnemonic_signing_material,
    protect_secret_signing_material, sign_protected_transaction_envelope,
    transaction_envelope_xdr, unlock_software_signer, ExportedSigningMaterial,
    ProtectedSignerError, ProtectedSigningError, ProtectionError, ProtectionRegistry,
    SecretStoreError, WalletMaterialError, WalletUnlockKey,
};

pub const CLIENT_API_VERSION: u64 = 1;

/// Transport-neutral Core boundary for process, mobile, desktop, and SDK hosts.
///
/// This layer deliberately owns no persistence, networking, system authentication,
/// or UI policy. Binding transports should adapt their native types to these
/// operations rather than reproducing wallet cryptography or error classification.
pub struct CoreClientApi {
    registry: ProtectionRegistry,
}

impl Default for CoreClientApi {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreClientApi {
    pub fn new() -> Self {
        Self {
            registry: ProtectionRegistry::new(),
        }
    }

    pub fn protect_secret(
        &self,
        secret: &str,
        passcode: &str,
    ) -> Result<ClientProtectedWallet, ClientApiError> {
        let protected = protect_secret_signing_material(&self.registry, secret, passcode)
            .map_err(classify_wallet_material_error)?;
        Ok(ClientProtectedWallet {
            public_key: protected.public_key,
            envelope: protected.envelope,
        })
    }

    pub fn protect_mnemonic(
        &self,
        mnemonic: &str,
        mnemonic_passphrase: &str,
        index: usize,
        language: Option<&str>,
        passcode: &str,
    ) -> Result<ClientProtectedWallet, ClientApiError> {
        let protected = protect_mnemonic_signing_material(
            &self.registry,
            mnemonic,
            mnemonic_passphrase,
            index,
            language,
            passcode,
        )
        .map_err(classify_wallet_material_error)?;
        Ok(ClientProtectedWallet {
            public_key: protected.public_key,
            envelope: protected.envelope,
        })
    }

    pub fn generate_mnemonic(
        &self,
        language: &str,
        strength: usize,
        mnemonic_passphrase: &str,
        index: usize,
        passcode: &str,
    ) -> Result<ClientGeneratedMnemonic, ClientApiError> {
        let generated = generate_protected_mnemonic(
            &self.registry,
            language,
            strength,
            mnemonic_passphrase,
            index,
            passcode,
        )
        .map_err(classify_wallet_material_error)?;
        Ok(ClientGeneratedMnemonic {
            wallet: ClientProtectedWallet {
                public_key: generated.wallet.public_key,
                envelope: generated.wallet.envelope,
            },
            mnemonic: generated.mnemonic,
            language: language.to_owned(),
            index,
        })
    }

    pub fn derive_unlock_key(
        &self,
        envelope: &Value,
        passcode: &str,
        expected_public_key: &str,
    ) -> Result<WalletUnlockKey, ClientApiError> {
        derive_verified_unlock_key(
            &self.registry,
            envelope,
            passcode,
            expected_public_key,
        )
        .map_err(classify_protected_signer_error)
    }

    pub fn validate_unlock_key(
        &self,
        envelope: &Value,
        unlock_key: &WalletUnlockKey,
        expected_public_key: &str,
    ) -> Result<(), ClientApiError> {
        let signer = unlock_software_signer(
            &self.registry,
            envelope,
            unlock_key,
            expected_public_key,
        )
        .map_err(classify_protected_signer_error)?;
        drop(signer);
        Ok(())
    }

    pub fn sign_transaction_xdr(
        &self,
        protected_envelope: &Value,
        unlock_key: &WalletUnlockKey,
        expected_public_key: &str,
        transaction_xdr: &[u8],
        network_passphrase: &str,
    ) -> Result<Vec<u8>, ClientApiError> {
        let mut transaction = parse_transaction_envelope_xdr(transaction_xdr)
            .map_err(|_| ClientApiError::invalid_transaction())?;
        sign_protected_transaction_envelope(
            &self.registry,
            protected_envelope,
            unlock_key,
            expected_public_key,
            &mut transaction,
            network_passphrase,
        )
        .map_err(classify_protected_signing_error)?;
        transaction_envelope_xdr(&transaction)
            .map_err(|_| ClientApiError::invalid_transaction())
    }

    pub fn reveal(
        &self,
        envelope: &Value,
        passcode: &str,
        expected_public_key: &str,
    ) -> Result<ExportedSigningMaterial, ClientApiError> {
        export_signing_material(
            &self.registry,
            envelope,
            passcode,
            expected_public_key,
        )
        .map_err(classify_protected_signer_error)
    }
}

pub struct ClientProtectedWallet {
    pub public_key: String,
    pub envelope: Value,
}

pub struct ClientGeneratedMnemonic {
    pub wallet: ClientProtectedWallet,
    pub mnemonic: Zeroizing<String>,
    pub language: String,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClientApiErrorCode {
    InvalidInput,
    InvalidPasscode,
    InvalidUnlockKey,
    InvalidProtectedData,
    IdentityMismatch,
    InvalidTransaction,
    CoreError,
}

impl ClientApiErrorCode {
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

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct ClientApiError {
    code: ClientApiErrorCode,
    message: String,
}

impl ClientApiError {
    pub fn code(&self) -> ClientApiErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            code: ClientApiErrorCode::InvalidInput,
            message: message.into(),
        }
    }

    fn invalid_passcode() -> Self {
        Self {
            code: ClientApiErrorCode::InvalidPasscode,
            message: "invalid wallet passcode".to_owned(),
        }
    }

    fn invalid_unlock_key() -> Self {
        Self {
            code: ClientApiErrorCode::InvalidUnlockKey,
            message: "invalid wallet unlock key".to_owned(),
        }
    }

    fn invalid_protected_data() -> Self {
        Self {
            code: ClientApiErrorCode::InvalidProtectedData,
            message: "protected wallet data is corrupted or unsupported".to_owned(),
        }
    }

    fn identity_mismatch() -> Self {
        Self {
            code: ClientApiErrorCode::IdentityMismatch,
            message: "wallet identity does not match metadata".to_owned(),
        }
    }

    fn invalid_transaction() -> Self {
        Self {
            code: ClientApiErrorCode::InvalidTransaction,
            message: "invalid Stellar transaction".to_owned(),
        }
    }

    fn core(message: impl Into<String>) -> Self {
        Self {
            code: ClientApiErrorCode::CoreError,
            message: message.into(),
        }
    }
}

fn classify_wallet_material_error(error: WalletMaterialError) -> ClientApiError {
    match error {
        WalletMaterialError::Signer(_) | WalletMaterialError::Derivation(_) => {
            ClientApiError::invalid_input(error.to_string())
        }
        WalletMaterialError::Protection(error) => classify_protection_error(error),
    }
}

fn classify_protected_signer_error(error: ProtectedSignerError) -> ClientApiError {
    match error {
        ProtectedSignerError::Protection(error) => classify_protection_error(error),
        ProtectedSignerError::InvalidExpectedPublicKey => {
            ClientApiError::invalid_input("expected_public_key is invalid")
        }
        ProtectedSignerError::IdentityMismatch => ClientApiError::identity_mismatch(),
        ProtectedSignerError::Signer(_)
        | ProtectedSignerError::Derivation(_)
        | ProtectedSignerError::InvalidSigningMaterial
        | ProtectedSignerError::UnsupportedSigningMaterial => {
            ClientApiError::invalid_protected_data()
        }
    }
}

fn classify_protected_signing_error(error: ProtectedSigningError) -> ClientApiError {
    match error {
        ProtectedSigningError::Unlock(error) => classify_protected_signer_error(error),
        ProtectedSigningError::Transaction(_) => ClientApiError::invalid_transaction(),
    }
}

fn classify_protection_error(error: ProtectionError) -> ClientApiError {
    match error {
        ProtectionError::SecretStore(error) => classify_secret_store_error(error),
        ProtectionError::UnsupportedProtectionKind(_)
        | ProtectionError::CorruptedMetadata
        | ProtectionError::UnsupportedFormat
        | ProtectionError::UnsupportedVersion => ClientApiError::invalid_protected_data(),
        ProtectionError::ProviderUnavailable(_) => ClientApiError::core(error.to_string()),
    }
}

fn classify_secret_store_error(error: SecretStoreError) -> ClientApiError {
    match error {
        SecretStoreError::EmptyPassword | SecretStoreError::InvalidPassword => {
            ClientApiError::invalid_passcode()
        }
        SecretStoreError::InvalidUnlockKey => ClientApiError::invalid_unlock_key(),
        SecretStoreError::UnsupportedEncryptionFormat
        | SecretStoreError::UnsupportedKdfFormat
        | SecretStoreError::UnsupportedKdfParameters
        | SecretStoreError::Corrupted
        | SecretStoreError::AuthenticationFailed => ClientApiError::invalid_protected_data(),
        SecretStoreError::RandomUnavailable | SecretStoreError::CryptoFailure => {
            ClientApiError::core(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
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
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn typed_api_roundtrips_secret_and_exact_transaction_vector() {
        let api = CoreClientApi::new();
        let protected = api.protect_secret(SECRET, "passcode").unwrap();
        assert_eq!(protected.public_key, PUBLIC);
        assert!(!protected.envelope.to_string().contains(SECRET));

        let unlock_key = api
            .derive_unlock_key(&protected.envelope, "passcode", PUBLIC)
            .unwrap();
        api.validate_unlock_key(&protected.envelope, &unlock_key, PUBLIC)
            .unwrap();
        let signed = api
            .sign_transaction_xdr(
                &protected.envelope,
                &unlock_key,
                PUBLIC,
                &decode_hex(UNSIGNED_XDR_HEX),
                TESTNET,
            )
            .unwrap();
        assert_eq!(signed, decode_hex(SIGNED_XDR_HEX));
    }

    #[test]
    fn typed_api_has_stable_authentication_error_categories() {
        let api = CoreClientApi::new();
        let protected = api.protect_secret(SECRET, "correct").unwrap();
        let wrong_passcode = api
            .derive_unlock_key(&protected.envelope, "wrong", PUBLIC)
            .unwrap_err();
        assert_eq!(wrong_passcode.code(), ClientApiErrorCode::InvalidPasscode);
        assert_eq!(wrong_passcode.code().as_str(), "invalid-passcode");

        let wrong_key = WalletUnlockKey::from_bytes([0u8; 32]);
        let error = api
            .validate_unlock_key(&protected.envelope, &wrong_key, PUBLIC)
            .unwrap_err();
        assert_eq!(error.code(), ClientApiErrorCode::InvalidUnlockKey);
    }

    #[test]
    fn typed_api_distinguishes_corrupted_protected_data() {
        let api = CoreClientApi::new();
        let error = api
            .derive_unlock_key(&json!({"format": "unknown"}), "passcode", PUBLIC)
            .unwrap_err();
        assert_eq!(error.code(), ClientApiErrorCode::InvalidProtectedData);
        assert_eq!(error.code().as_str(), "invalid-protected-data");
    }

    #[test]
    fn typed_api_distinguishes_invalid_transaction() {
        let api = CoreClientApi::new();
        let protected = api.protect_secret(SECRET, "passcode").unwrap();
        let key = api
            .derive_unlock_key(&protected.envelope, "passcode", PUBLIC)
            .unwrap();
        let error = api
            .sign_transaction_xdr(&protected.envelope, &key, PUBLIC, b"not-xdr", TESTNET)
            .unwrap_err();
        assert_eq!(error.code(), ClientApiErrorCode::InvalidTransaction);
    }
}

use serde_json::Value;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    derive_verified_unlock_key, export_signing_material, generate_protected_mnemonic,
    parse_transaction_envelope_xdr, protect_mnemonic_signing_material,
    protect_secret_signing_material, sign_protected_transaction_envelope,
    sign_transaction_envelope, transaction_envelope_xdr, transaction_hash,
    unlock_software_signer, AccountIdentity, AccountKind, ExportedSigningMaterial,
    ExternalEd25519Signer, ProtectedSignerError, ProtectedSigningError, ProtectionError,
    ProtectionRegistry, SecretStoreError, SignerError, TransactionSigningError,
    WalletMaterialError, WalletUnlockKey,
};

pub const CLIENT_API_VERSION: u64 = 2;

/// Transport-neutral Core boundary for process, mobile, desktop, and SDK hosts.
///
/// This layer deliberately owns no persistence, networking, system authentication,
/// account-to-signer ledger policy, or UI policy. Binding transports should adapt
/// native types to these operations rather than reproducing wallet cryptography,
/// identity checks, transaction hashing, signature verification, or error mapping.
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

    pub fn parse_account(&self, address: &str) -> Result<ClientAccountIdentity, ClientApiError> {
        let identity = AccountIdentity::parse(address)
            .map_err(|error| ClientApiError::invalid_input(error.to_string()))?;
        Ok(ClientAccountIdentity {
            kind: match identity.kind() {
                AccountKind::Classic => ClientAccountKind::Classic,
                AccountKind::Contract => ClientAccountKind::Contract,
            },
            address: identity.address().to_owned(),
            public_key: identity.public_key().map(str::to_owned),
        })
    }

    pub fn protect_secret(
        &self,
        secret: &str,
        passcode: &str,
        expected_signer_public_key: Option<&str>,
    ) -> Result<ClientProtectedSoftwareSigner, ClientApiError> {
        let protected = protect_secret_signing_material(&self.registry, secret, passcode)
            .map_err(classify_wallet_material_error)?;
        ensure_expected_signer_public_key(
            &protected.public_key,
            expected_signer_public_key,
        )?;
        Ok(ClientProtectedSoftwareSigner {
            signer_public_key: protected.public_key,
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
        expected_signer_public_key: Option<&str>,
    ) -> Result<ClientProtectedSoftwareSigner, ClientApiError> {
        let protected = protect_mnemonic_signing_material(
            &self.registry,
            mnemonic,
            mnemonic_passphrase,
            index,
            language,
            passcode,
        )
        .map_err(classify_wallet_material_error)?;
        ensure_expected_signer_public_key(
            &protected.public_key,
            expected_signer_public_key,
        )?;
        Ok(ClientProtectedSoftwareSigner {
            signer_public_key: protected.public_key,
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
            signer: ClientProtectedSoftwareSigner {
                signer_public_key: generated.wallet.public_key,
                envelope: generated.wallet.envelope,
            },
            mnemonic: generated.mnemonic,
            language: language.to_owned(),
            index,
        })
    }

    pub fn reprotect(
        &self,
        envelope: &Value,
        current_passcode: &str,
        new_passcode: &str,
        expected_signer_public_key: &str,
    ) -> Result<ClientProtectedSoftwareSigner, ClientApiError> {
        let material = self.reveal(
            envelope,
            current_passcode,
            expected_signer_public_key,
        )?;
        match material {
            ExportedSigningMaterial::Secret { secret } => self.protect_secret(
                secret.as_str(),
                new_passcode,
                Some(expected_signer_public_key),
            ),
            ExportedSigningMaterial::Mnemonic {
                mnemonic,
                mnemonic_passphrase,
                index,
                language,
            } => self.protect_mnemonic(
                mnemonic.as_str(),
                mnemonic_passphrase.as_str(),
                index,
                Some(language.as_str()),
                new_passcode,
                Some(expected_signer_public_key),
            ),
        }
    }

    pub fn derive_unlock_key(
        &self,
        envelope: &Value,
        passcode: &str,
        expected_signer_public_key: &str,
    ) -> Result<WalletUnlockKey, ClientApiError> {
        derive_verified_unlock_key(
            &self.registry,
            envelope,
            passcode,
            expected_signer_public_key,
        )
        .map_err(classify_protected_signer_error)
    }

    pub fn validate_unlock_key(
        &self,
        envelope: &Value,
        unlock_key: &WalletUnlockKey,
        expected_signer_public_key: &str,
    ) -> Result<(), ClientApiError> {
        let signer = unlock_software_signer(
            &self.registry,
            envelope,
            unlock_key,
            expected_signer_public_key,
        )
        .map_err(classify_protected_signer_error)?;
        drop(signer);
        Ok(())
    }

    pub fn sign_transaction_xdr(
        &self,
        protected_envelope: &Value,
        unlock_key: &WalletUnlockKey,
        expected_signer_public_key: &str,
        transaction_xdr: &[u8],
        network_passphrase: &str,
    ) -> Result<Vec<u8>, ClientApiError> {
        let mut transaction = parse_transaction_envelope_xdr(transaction_xdr)
            .map_err(|_| ClientApiError::invalid_transaction())?;
        sign_protected_transaction_envelope(
            &self.registry,
            protected_envelope,
            unlock_key,
            expected_signer_public_key,
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
        expected_signer_public_key: &str,
    ) -> Result<ExportedSigningMaterial, ClientApiError> {
        export_signing_material(
            &self.registry,
            envelope,
            passcode,
            expected_signer_public_key,
        )
        .map_err(classify_protected_signer_error)
    }

    pub fn prepare_ed25519_signing(
        &self,
        transaction_xdr: &[u8],
        network_passphrase: &str,
    ) -> Result<ClientEd25519SigningRequest, ClientApiError> {
        let transaction = parse_transaction_envelope_xdr(transaction_xdr)
            .map_err(|_| ClientApiError::invalid_transaction())?;
        let transaction_hash = transaction_hash(&transaction, network_passphrase)
            .map_err(|_| ClientApiError::invalid_transaction())?;
        let transaction_xdr = transaction_envelope_xdr(&transaction)
            .map_err(|_| ClientApiError::invalid_transaction())?;
        Ok(ClientEd25519SigningRequest {
            transaction_hash,
            transaction_xdr,
            network_passphrase: network_passphrase.to_owned(),
        })
    }

    pub fn apply_ed25519_signature(
        &self,
        transaction_xdr: &[u8],
        network_passphrase: &str,
        signer_public_key: &str,
        signature: &[u8],
    ) -> Result<Vec<u8>, ClientApiError> {
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| ClientApiError::invalid_input("signature must be 64 bytes"))?;
        let signer = ExternalEd25519Signer::new(signer_public_key, move |_| Ok(signature))
            .map_err(classify_external_signer_error)?;
        let mut transaction = parse_transaction_envelope_xdr(transaction_xdr)
            .map_err(|_| ClientApiError::invalid_transaction())?;
        sign_transaction_envelope(&mut transaction, network_passphrase, &signer)
            .map_err(classify_external_transaction_error)?;
        transaction_envelope_xdr(&transaction)
            .map_err(|_| ClientApiError::invalid_transaction())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientAccountKind {
    Classic,
    Contract,
}

impl ClientAccountKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Contract => "contract",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientAccountIdentity {
    pub kind: ClientAccountKind,
    pub address: String,
    pub public_key: Option<String>,
}

pub struct ClientProtectedSoftwareSigner {
    pub signer_public_key: String,
    pub envelope: Value,
}

pub struct ClientGeneratedMnemonic {
    pub signer: ClientProtectedSoftwareSigner,
    pub mnemonic: Zeroizing<String>,
    pub language: String,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientEd25519SigningRequest {
    pub transaction_hash: [u8; 32],
    pub transaction_xdr: Vec<u8>,
    pub network_passphrase: String,
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
            message: "protected signer data is corrupted or unsupported".to_owned(),
        }
    }

    fn identity_mismatch() -> Self {
        Self {
            code: ClientApiErrorCode::IdentityMismatch,
            message: "signer identity does not match expected signer".to_owned(),
        }
    }

    fn invalid_transaction() -> Self {
        Self {
            code: ClientApiErrorCode::InvalidTransaction,
            message: "invalid Stellar transaction or signature".to_owned(),
        }
    }

    fn core(message: impl Into<String>) -> Self {
        Self {
            code: ClientApiErrorCode::CoreError,
            message: message.into(),
        }
    }
}

fn ensure_expected_signer_public_key(
    actual_public_key: &str,
    expected_signer_public_key: Option<&str>,
) -> Result<(), ClientApiError> {
    let Some(expected) = expected_signer_public_key else {
        return Ok(());
    };
    let expected = AccountIdentity::parse(expected)
        .map_err(|_| ClientApiError::invalid_input("expected_signer_public_key is invalid"))?;
    if !expected.is_classic() {
        return Err(ClientApiError::invalid_input(
            "expected_signer_public_key must be a classic G address",
        ));
    }
    if expected.public_key() != Some(actual_public_key) {
        return Err(ClientApiError::identity_mismatch());
    }
    Ok(())
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
            ClientApiError::invalid_input("expected_signer_public_key is invalid")
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

fn classify_external_signer_error(error: SignerError) -> ClientApiError {
    match error {
        SignerError::InvalidPublicKey => ClientApiError::invalid_input("signer_public_key is invalid"),
        SignerError::InvalidSecret => ClientApiError::invalid_input(error.to_string()),
        SignerError::ExternalProvider(_) => ClientApiError::core(error.to_string()),
    }
}

fn classify_external_transaction_error(error: TransactionSigningError) -> ClientApiError {
    match error {
        TransactionSigningError::Signer(error) => classify_external_signer_error(error),
        TransactionSigningError::Xdr(_)
        | TransactionSigningError::InvalidSignature
        | TransactionSigningError::DuplicateSignature => ClientApiError::invalid_transaction(),
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
    const OTHER_PUBLIC: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";
    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
    const TESTNET: &str = "Test SDF Network ; September 2015";
    const TRANSACTION_HASH_HEX: &str =
        "dd8d4e2abf55d45c62805bfaae02baf1143f8c79b457dc0db6e1887902f9e43e";
    const SIGNATURE_HEX: &str = concat!(
        "99254edb377824d7162192be9a4afc95e1943598051022de0e64fb1e75c75b43",
        "6e7cf492d41a6f3445728b4afbf640ec3d472f22141b5d1fdf1520c0ed758d09"
    );
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
    fn parses_classic_and_contract_account_identity() {
        let api = CoreClientApi::new();
        let classic = api.parse_account(PUBLIC).unwrap();
        assert_eq!(classic.kind, ClientAccountKind::Classic);
        assert_eq!(classic.public_key.as_deref(), Some(PUBLIC));

        let contract = api.parse_account(CONTRACT).unwrap();
        assert_eq!(contract.kind, ClientAccountKind::Contract);
        assert_eq!(contract.public_key, None);
    }

    #[test]
    fn typed_api_roundtrips_secret_and_exact_transaction_vector() {
        let api = CoreClientApi::new();
        let protected = api.protect_secret(SECRET, "passcode", None).unwrap();
        assert_eq!(protected.signer_public_key, PUBLIC);
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
    fn signer_attachment_rejects_wrong_expected_identity() {
        let api = CoreClientApi::new();
        let error = api
            .protect_secret(SECRET, "passcode", Some(OTHER_PUBLIC))
            .err()
            .unwrap();
        assert_eq!(error.code(), ClientApiErrorCode::IdentityMismatch);
    }

    #[test]
    fn reprotect_changes_unlock_key_without_exporting_material() {
        let api = CoreClientApi::new();
        let protected = api.protect_secret(SECRET, "old", Some(PUBLIC)).unwrap();
        let old_key = api
            .derive_unlock_key(&protected.envelope, "old", PUBLIC)
            .unwrap();
        let reprotected = api
            .reprotect(&protected.envelope, "old", "new", PUBLIC)
            .unwrap();
        assert_eq!(reprotected.signer_public_key, PUBLIC);
        assert_ne!(reprotected.envelope, protected.envelope);

        let new_key = api
            .derive_unlock_key(&reprotected.envelope, "new", PUBLIC)
            .unwrap();
        assert_ne!(new_key.as_bytes(), old_key.as_bytes());
        assert_eq!(
            api.derive_unlock_key(&reprotected.envelope, "old", PUBLIC)
                .unwrap_err()
                .code(),
            ClientApiErrorCode::InvalidPasscode
        );
    }

    #[test]
    fn external_signing_roundtrips_exact_vector() {
        let api = CoreClientApi::new();
        let prepared = api
            .prepare_ed25519_signing(&decode_hex(UNSIGNED_XDR_HEX), TESTNET)
            .unwrap();
        assert_eq!(prepared.transaction_hash.to_vec(), decode_hex(TRANSACTION_HASH_HEX));
        assert_eq!(prepared.transaction_xdr, decode_hex(UNSIGNED_XDR_HEX));

        let signed = api
            .apply_ed25519_signature(
                &prepared.transaction_xdr,
                TESTNET,
                PUBLIC,
                &decode_hex(SIGNATURE_HEX),
            )
            .unwrap();
        assert_eq!(signed, decode_hex(SIGNED_XDR_HEX));
    }

    #[test]
    fn typed_api_has_stable_authentication_error_categories() {
        let api = CoreClientApi::new();
        let protected = api.protect_secret(SECRET, "correct", None).unwrap();
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
        let protected = api.protect_secret(SECRET, "passcode", None).unwrap();
        let key = api
            .derive_unlock_key(&protected.envelope, "passcode", PUBLIC)
            .unwrap();
        let error = api
            .sign_transaction_xdr(&protected.envelope, &key, PUBLIC, b"not-xdr", TESTNET)
            .unwrap_err();
        assert_eq!(error.code(), ClientApiErrorCode::InvalidTransaction);
    }
}

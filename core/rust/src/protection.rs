use std::collections::HashMap;
use std::fmt;

use serde_json::{json, Value};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::secret_store::{
    decrypt_secret, decrypt_secret_with_unlock_key, derive_unlock_key, encrypt_secret,
    PasswordSecretEnvelope, SecretStoreError, WalletUnlockKey,
};

pub const PROTECTED_SECRET_FORMAT: &str = "fresnica-protected-secret";
pub const PROTECTED_SECRET_VERSION: u64 = 1;

pub enum ProtectionCredential {
    Password(Zeroizing<String>),
}

impl ProtectionCredential {
    pub fn password(password: impl Into<String>) -> Self {
        Self::Password(Zeroizing::new(password.into()))
    }

    pub fn kind(&self) -> &'static str {
        "password"
    }

    fn password_value(&self) -> &str {
        match self {
            Self::Password(password) => password.as_str(),
        }
    }
}

impl fmt::Debug for ProtectionCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Password")
            .field(&"<redacted>")
            .finish()
    }
}

pub trait ProtectionProvider {
    fn kind(&self) -> &'static str;

    fn protect(
        &self,
        payload: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError>;

    fn unprotect(
        &self,
        envelope: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError>;
}

#[derive(Default)]
pub struct PasswordProtectionProvider;

impl ProtectionProvider for PasswordProtectionProvider {
    fn kind(&self) -> &'static str {
        "password"
    }

    fn protect(
        &self,
        payload: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError> {
        serde_json::to_value(encrypt_secret(payload, credential.password_value())?)
            .map_err(|_| ProtectionError::CorruptedMetadata)
    }

    fn unprotect(
        &self,
        envelope: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError> {
        let envelope: PasswordSecretEnvelope = serde_json::from_value(envelope.clone())
            .map_err(|_| ProtectionError::CorruptedMetadata)?;
        decrypt_secret(&envelope, credential.password_value()).map_err(Into::into)
    }
}

pub struct ProtectionRegistry {
    providers: HashMap<String, Box<dyn ProtectionProvider>>,
}

impl Default for ProtectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtectionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };
        registry.register(PasswordProtectionProvider);
        registry
    }

    pub fn register<P>(&mut self, provider: P)
    where
        P: ProtectionProvider + 'static,
    {
        self.providers
            .insert(provider.kind().to_owned(), Box::new(provider));
    }

    pub fn kind_for(&self, envelope: &Value) -> Result<String, ProtectionError> {
        let object = envelope
            .as_object()
            .ok_or(ProtectionError::CorruptedMetadata)?;

        if object.get("format").and_then(Value::as_str) == Some(PROTECTED_SECRET_FORMAT) {
            if object.get("version").and_then(Value::as_u64) != Some(PROTECTED_SECRET_VERSION) {
                return Err(ProtectionError::UnsupportedVersion);
            }
            let kind = object
                .get("protection")
                .and_then(Value::as_object)
                .and_then(|protection| protection.get("type"))
                .and_then(Value::as_str)
                .ok_or(ProtectionError::CorruptedMetadata)?;
            return Ok(kind.to_owned());
        }

        if Self::is_legacy_password(envelope) {
            return Ok("password".to_owned());
        }

        Err(ProtectionError::UnsupportedFormat)
    }

    pub fn protect(
        &self,
        payload: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError> {
        let provider = self.provider("password")?;
        let provider_envelope = provider.protect(payload, credential)?;
        self.wrap("password", provider_envelope)
    }

    pub fn unprotect(
        &self,
        envelope: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError> {
        let provider_envelope = self.password_provider_envelope(envelope)?;
        self.provider("password")?
            .unprotect(provider_envelope, credential)
    }

    pub fn derive_unlock_key(
        &self,
        envelope: &Value,
        password: &str,
    ) -> Result<WalletUnlockKey, ProtectionError> {
        let provider_envelope = self.password_provider_envelope(envelope)?;
        let envelope: PasswordSecretEnvelope = serde_json::from_value(provider_envelope.clone())
            .map_err(|_| ProtectionError::CorruptedMetadata)?;
        derive_unlock_key(&envelope, password).map_err(Into::into)
    }

    pub fn unprotect_with_unlock_key(
        &self,
        envelope: &Value,
        unlock_key: &WalletUnlockKey,
    ) -> Result<Value, ProtectionError> {
        let provider_envelope = self.password_provider_envelope(envelope)?;
        let envelope: PasswordSecretEnvelope = serde_json::from_value(provider_envelope.clone())
            .map_err(|_| ProtectionError::CorruptedMetadata)?;
        decrypt_secret_with_unlock_key(&envelope, unlock_key).map_err(Into::into)
    }

    pub fn migrate_legacy_password(&self, envelope: &Value) -> Result<Value, ProtectionError> {
        if !Self::is_legacy_password(envelope) {
            return Ok(envelope.clone());
        }
        self.wrap("password", envelope.clone())
    }

    pub fn is_legacy_password(envelope: &Value) -> bool {
        envelope.as_object().is_some_and(|object| {
            object.get("format").and_then(Value::as_str) != Some(PROTECTED_SECRET_FORMAT)
                && object.get("cipher").and_then(Value::as_str) == Some("aes-256-gcm")
                && object.contains_key("kdf")
        })
    }

    fn password_provider_envelope<'a>(
        &self,
        envelope: &'a Value,
    ) -> Result<&'a Value, ProtectionError> {
        let kind = self.kind_for(envelope)?;
        if kind != "password" {
            return Err(ProtectionError::UnsupportedProtectionKind(kind));
        }

        if Self::is_legacy_password(envelope) {
            return Ok(envelope);
        }

        envelope
            .as_object()
            .and_then(|object| object.get("payload"))
            .ok_or(ProtectionError::CorruptedMetadata)
    }

    fn wrap(&self, kind: &str, provider_envelope: Value) -> Result<Value, ProtectionError> {
        self.provider(kind)?;
        Ok(json!({
            "format": PROTECTED_SECRET_FORMAT,
            "version": PROTECTED_SECRET_VERSION,
            "protection": {"type": kind},
            "payload": provider_envelope,
        }))
    }

    fn provider(&self, kind: &str) -> Result<&dyn ProtectionProvider, ProtectionError> {
        self.providers
            .get(kind)
            .map(Box::as_ref)
            .ok_or_else(|| ProtectionError::ProviderUnavailable(kind.to_owned()))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtectionError {
    #[error(transparent)]
    SecretStore(#[from] SecretStoreError),
    #[error("protection provider is not available: {0}")]
    ProviderUnavailable(String),
    #[error("unsupported wallet protection kind: {0}")]
    UnsupportedProtectionKind(String),
    #[error("wallet protection metadata is corrupted")]
    CorruptedMetadata,
    #[error("unsupported wallet protection format")]
    UnsupportedFormat,
    #[error("unsupported wallet protection version")]
    UnsupportedVersion,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn password_provider_roundtrips_through_registry() {
        let registry = ProtectionRegistry::new();
        let payload = json!({"kind": "secret", "secret": "S..."});
        let credential = ProtectionCredential::password("correct");
        let envelope = registry.protect(&payload, &credential).unwrap();

        assert_eq!(registry.kind_for(&envelope).unwrap(), "password");
        assert!(!envelope.to_string().contains("S..."));
        assert_eq!(registry.unprotect(&envelope, &credential).unwrap(), payload);
    }

    #[test]
    fn same_envelope_opens_with_derived_unlock_key() {
        let registry = ProtectionRegistry::new();
        let payload = json!({"kind": "secret", "secret": "S..."});
        let credential = ProtectionCredential::password("correct");
        let envelope = registry.protect(&payload, &credential).unwrap();
        let key = registry.derive_unlock_key(&envelope, "correct").unwrap();

        assert_eq!(registry.unprotect_with_unlock_key(&envelope, &key).unwrap(), payload);
    }

    #[test]
    fn wrong_unlock_key_is_rejected() {
        let registry = ProtectionRegistry::new();
        let payload = json!({"kind": "secret", "secret": "S..."});
        let envelope = registry
            .protect(&payload, &ProtectionCredential::password("correct"))
            .unwrap();
        let wrong = WalletUnlockKey::from_bytes([0u8; 32]);

        assert_eq!(
            registry.unprotect_with_unlock_key(&envelope, &wrong).unwrap_err(),
            ProtectionError::SecretStore(SecretStoreError::InvalidUnlockKey)
        );
    }

    #[test]
    fn legacy_password_envelope_is_readable_and_migratable() {
        let provider = PasswordProtectionProvider;
        let credential = ProtectionCredential::password("correct");
        let payload = json!({"kind": "secret", "secret": "S..."});
        let legacy = provider.protect(&payload, &credential).unwrap();
        let registry = ProtectionRegistry::new();

        assert_eq!(registry.kind_for(&legacy).unwrap(), "password");
        assert_eq!(registry.unprotect(&legacy, &credential).unwrap(), payload);
        let migrated = registry.migrate_legacy_password(&legacy).unwrap();
        assert_eq!(registry.kind_for(&migrated).unwrap(), "password");
        assert_eq!(migrated["payload"], legacy);
        assert_eq!(registry.unprotect(&migrated, &credential).unwrap(), payload);
    }
}

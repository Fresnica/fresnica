use std::collections::HashMap;
use std::fmt;

use serde_json::{json, Value};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::secret_store::{
    decrypt_secret, decrypt_secret_with_key, encrypt_secret, encrypt_secret_with_key,
    KeySecretEnvelope, PasswordSecretEnvelope, SecretStoreError,
};

pub const PROTECTED_SECRET_FORMAT: &str = "fresnica-protected-secret";
pub const PROTECTED_SECRET_VERSION: u64 = 1;

pub enum ProtectionCredential {
    Password(Zeroizing<String>),
    System,
}

impl ProtectionCredential {
    pub fn password(password: impl Into<String>) -> Self {
        Self::Password(Zeroizing::new(password.into()))
    }

    pub fn system() -> Self {
        Self::System
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Password(_) => "password",
            Self::System => "system",
        }
    }

    fn password_value(&self) -> Option<&str> {
        match self {
            Self::Password(password) => Some(password.as_str()),
            Self::System => None,
        }
    }
}

impl fmt::Debug for ProtectionCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password(_) => formatter.debug_tuple("Password").field(&"<redacted>").finish(),
            Self::System => formatter.write_str("System"),
        }
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
        let password = require_password(credential)?;
        serde_json::to_value(encrypt_secret(payload, password)?)
            .map_err(|_| ProtectionError::CorruptedMetadata)
    }

    fn unprotect(
        &self,
        envelope: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError> {
        let password = require_password(credential)?;
        let envelope: PasswordSecretEnvelope = serde_json::from_value(envelope.clone())
            .map_err(|_| ProtectionError::CorruptedMetadata)?;
        decrypt_secret(&envelope, password).map_err(Into::into)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("system key store operation failed")]
pub struct SystemKeyStoreError;

pub trait SystemKeyStore {
    fn store_key(&self, key: &[u8; 32]) -> Result<String, SystemKeyStoreError>;
    fn load_key(&self, reference: &str) -> Result<Zeroizing<[u8; 32]>, SystemKeyStoreError>;
}

pub struct SystemProtectionProvider {
    key_store: Box<dyn SystemKeyStore>,
}

impl SystemProtectionProvider {
    pub fn new<K>(key_store: K) -> Self
    where
        K: SystemKeyStore + 'static,
    {
        Self {
            key_store: Box::new(key_store),
        }
    }
}

impl ProtectionProvider for SystemProtectionProvider {
    fn kind(&self) -> &'static str {
        "system"
    }

    fn protect(
        &self,
        payload: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError> {
        require_kind(credential, self.kind())?;
        let mut key = Zeroizing::new([0u8; 32]);
        getrandom::fill(&mut key[..]).map_err(|_| ProtectionError::SystemUnavailable(
            "system protection could not generate a wallet protection key".to_owned(),
        ))?;
        let reference = self.key_store.store_key(&key).map_err(|_| {
            ProtectionError::SystemUnavailable(
                "system protection could not store a wallet protection key".to_owned(),
            )
        })?;
        if reference.is_empty() {
            return Err(ProtectionError::InvalidKeyReference);
        }
        let secret = encrypt_secret_with_key(payload, &key)?;
        Ok(json!({
            "key_reference": reference,
            "secret": secret,
        }))
    }

    fn unprotect(
        &self,
        envelope: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError> {
        require_kind(credential, self.kind())?;
        let object = envelope
            .as_object()
            .ok_or(ProtectionError::CorruptedMetadata)?;
        let reference = object
            .get("key_reference")
            .and_then(Value::as_str)
            .filter(|reference| !reference.is_empty())
            .ok_or(ProtectionError::CorruptedMetadata)?;
        let secret: KeySecretEnvelope = serde_json::from_value(
            object
                .get("secret")
                .cloned()
                .ok_or(ProtectionError::CorruptedMetadata)?,
        )
        .map_err(|_| ProtectionError::CorruptedMetadata)?;
        let key = self.key_store.load_key(reference).map_err(|_| {
            ProtectionError::SystemUnavailable(
                "system protection could not access the wallet protection key".to_owned(),
            )
        })?;
        decrypt_secret_with_key(&secret, &key).map_err(Into::into)
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
        let kind = credential.kind();
        let provider = self.provider(kind)?;
        let provider_envelope = provider.protect(payload, credential)?;
        self.wrap(kind, provider_envelope)
    }

    pub fn unprotect(
        &self,
        envelope: &Value,
        credential: &ProtectionCredential,
    ) -> Result<Value, ProtectionError> {
        let kind = self.kind_for(envelope)?;
        if credential.kind() != kind {
            return Err(ProtectionError::CredentialMismatch {
                expected: kind,
                actual: credential.kind().to_owned(),
            });
        }
        let provider = self.provider(&kind)?;
        if Self::is_legacy_password(envelope) {
            return provider.unprotect(envelope, credential);
        }
        let payload = envelope
            .as_object()
            .and_then(|object| object.get("payload"))
            .ok_or(ProtectionError::CorruptedMetadata)?;
        provider.unprotect(payload, credential)
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

fn require_password(credential: &ProtectionCredential) -> Result<&str, ProtectionError> {
    credential
        .password_value()
        .ok_or_else(|| ProtectionError::CredentialMismatch {
            expected: "password".to_owned(),
            actual: credential.kind().to_owned(),
        })
}

fn require_kind(
    credential: &ProtectionCredential,
    expected: &str,
) -> Result<(), ProtectionError> {
    if credential.kind() == expected {
        return Ok(());
    }
    Err(ProtectionError::CredentialMismatch {
        expected: expected.to_owned(),
        actual: credential.kind().to_owned(),
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtectionError {
    #[error(transparent)]
    SecretStore(#[from] SecretStoreError),
    #[error("protection credential {actual:?} cannot unlock {expected:?} material")]
    CredentialMismatch { expected: String, actual: String },
    #[error("protection provider is not available: {0}")]
    ProviderUnavailable(String),
    #[error("system protection is unavailable: {0}")]
    SystemUnavailable(String),
    #[error("system protection returned an invalid key reference")]
    InvalidKeyReference,
    #[error("wallet protection metadata is corrupted")]
    CorruptedMetadata,
    #[error("unsupported wallet protection format")]
    UnsupportedFormat,
    #[error("unsupported wallet protection version")]
    UnsupportedVersion,
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;

    use serde_json::json;

    use super::*;

    #[derive(Clone, Default)]
    struct MemoryKeyStore {
        state: Rc<MemoryKeyStoreState>,
    }

    #[derive(Default)]
    struct MemoryKeyStoreState {
        keys: RefCell<HashMap<String, [u8; 32]>>,
        next_id: Cell<usize>,
    }

    impl SystemKeyStore for MemoryKeyStore {
        fn store_key(&self, key: &[u8; 32]) -> Result<String, SystemKeyStoreError> {
            let reference = format!("key-{}", self.state.next_id.get());
            self.state.next_id.set(self.state.next_id.get() + 1);
            self.state.keys.borrow_mut().insert(reference.clone(), *key);
            Ok(reference)
        }

        fn load_key(&self, reference: &str) -> Result<Zeroizing<[u8; 32]>, SystemKeyStoreError> {
            self.state
                .keys
                .borrow()
                .get(reference)
                .copied()
                .map(Zeroizing::new)
                .ok_or(SystemKeyStoreError)
        }
    }

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

    #[test]
    fn system_provider_keeps_wrapping_key_outside_envelope() {
        let key_store = MemoryKeyStore::default();
        let inspect = key_store.clone();
        let mut registry = ProtectionRegistry::new();
        registry.register(SystemProtectionProvider::new(key_store));
        let payload = json!({"kind": "secret", "secret": "S..."});
        let credential = ProtectionCredential::system();
        let envelope = registry.protect(&payload, &credential).unwrap();

        assert_eq!(registry.kind_for(&envelope).unwrap(), "system");
        assert!(!envelope.to_string().contains("S..."));
        assert_eq!(inspect.state.keys.borrow().len(), 1);
        assert_eq!(registry.unprotect(&envelope, &credential).unwrap(), payload);
    }

    #[test]
    fn registry_rejects_wrong_credential_kind() {
        let key_store = MemoryKeyStore::default();
        let mut registry = ProtectionRegistry::new();
        registry.register(SystemProtectionProvider::new(key_store));
        let envelope = registry
            .protect(&json!({"secret": "S..."}), &ProtectionCredential::system())
            .unwrap();

        assert!(matches!(
            registry.unprotect(&envelope, &ProtectionCredential::password("irrelevant")),
            Err(ProtectionError::CredentialMismatch { .. })
        ));
    }

    #[test]
    fn system_key_store_failure_maps_to_system_unavailable() {
        let key_store = MemoryKeyStore::default();
        let mut registry = ProtectionRegistry::new();
        registry.register(SystemProtectionProvider::new(key_store));
        let mut envelope = registry
            .protect(&json!({"secret": "S..."}), &ProtectionCredential::system())
            .unwrap();
        envelope["payload"]["key_reference"] = Value::String("missing".to_owned());

        assert!(matches!(
            registry.unprotect(&envelope, &ProtectionCredential::system()),
            Err(ProtectionError::SystemUnavailable(message))
                if message == "system protection could not access the wallet protection key"
        ));
    }
}

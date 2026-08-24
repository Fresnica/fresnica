use std::io::{self, Read, Write};
use std::process;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_core::{
    derive_verified_unlock_key, export_signing_material, generate_protected_mnemonic,
    parse_transaction_envelope_xdr, protect_mnemonic_signing_material,
    protect_secret_signing_material, sign_protected_transaction_envelope,
    transaction_envelope_xdr, ExportedSigningMaterial, ProtectedSignerError,
    ProtectedSigningError, ProtectionError, ProtectionRegistry, SecretStoreError,
    WalletMaterialError, WalletUnlockKey,
};
use serde_json::{json, Map, Value};
use zeroize::Zeroizing;

const PROTOCOL_VERSION: u64 = 1;

fn main() {
    match run() {
        Ok(result) => write_response(
            json!({
                "ok": true,
                "protocol_version": PROTOCOL_VERSION,
                "result": result,
            }),
            0,
        ),
        Err(error) => write_response(
            json!({
                "ok": false,
                "protocol_version": PROTOCOL_VERSION,
                "error": {
                    "code": error.code(),
                    "message": error.message(),
                },
            }),
            2,
        ),
    }
}

fn write_response(response: Value, exit_code: i32) -> ! {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if serde_json::to_writer(&mut output, &response).is_err() || output.write_all(b"\n").is_err() {
        process::exit(3);
    }
    process::exit(exit_code);
}

fn run() -> Result<Value, BridgeError> {
    let mut input = Zeroizing::new(String::new());
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|_| BridgeError::InvalidRequest("unable to read request"))?;
    let mut request: Value = serde_json::from_str(&input)
        .map_err(|_| BridgeError::InvalidRequest("request must be valid JSON"))?;
    let object = request
        .as_object_mut()
        .ok_or(BridgeError::InvalidRequest("request must be a JSON object"))?;
    let operation = take_string(object, "operation")?;
    let registry = ProtectionRegistry::new();

    match operation.as_str() {
        "version" => Ok(json!({
            "core_version": env!("CARGO_PKG_VERSION"),
            "protocol_version": PROTOCOL_VERSION,
        })),
        "protect-secret" => {
            let secret = take_sensitive_string(object, "secret")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let protected = protect_secret_signing_material(&registry, &secret, &passcode)
                .map_err(classify_wallet_material_error)?;
            Ok(json!({
                "public_key": protected.public_key,
                "envelope": protected.envelope,
            }))
        }
        "protect-mnemonic" => {
            let mnemonic = take_sensitive_string(object, "mnemonic")?;
            let mnemonic_passphrase = take_optional_sensitive_string(
                object,
                "mnemonic_passphrase",
            )?;
            let index = take_optional_usize(object, "index", 0)?;
            let language = take_optional_string(object, "language")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let protected = protect_mnemonic_signing_material(
                &registry,
                &mnemonic,
                &mnemonic_passphrase,
                index,
                language.as_deref(),
                &passcode,
            )
            .map_err(classify_wallet_material_error)?;
            Ok(json!({
                "public_key": protected.public_key,
                "envelope": protected.envelope,
            }))
        }
        "generate-mnemonic" => {
            let language = take_optional_string(object, "language")?
                .unwrap_or_else(|| "english".to_owned());
            let strength = take_optional_usize(object, "strength", 256)?;
            let index = take_optional_usize(object, "index", 0)?;
            let mnemonic_passphrase = take_optional_sensitive_string(
                object,
                "mnemonic_passphrase",
            )?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let generated = generate_protected_mnemonic(
                &registry,
                &language,
                strength,
                &mnemonic_passphrase,
                index,
                &passcode,
            )
            .map_err(classify_wallet_material_error)?;
            Ok(json!({
                "public_key": generated.wallet.public_key,
                "envelope": generated.wallet.envelope,
                "mnemonic": generated.mnemonic.as_str(),
                "language": language,
                "index": index,
            }))
        }
        "derive-unlock-key" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected_public_key = take_string(object, "expected_public_key")?;
            let unlock_key = derive_verified_unlock_key(
                &registry,
                &envelope,
                &passcode,
                &expected_public_key,
            )
            .map_err(classify_protected_signer_error)?;
            Ok(json!({
                "unlock_key": STANDARD.encode(unlock_key.as_bytes()),
            }))
        }
        "sign-transaction" => {
            let envelope = take_value(object, "envelope")?;
            let unlock_key = decode_unlock_key(&take_sensitive_string(object, "unlock_key")?)?;
            let expected_public_key = take_string(object, "expected_public_key")?;
            let transaction_xdr = take_sensitive_string(object, "transaction_xdr")?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let raw_xdr = STANDARD
                .decode(transaction_xdr.as_bytes())
                .map_err(|_| BridgeError::InvalidRequest("transaction_xdr must be base64 XDR"))?;
            let mut transaction = parse_transaction_envelope_xdr(&raw_xdr)
                .map_err(|_| BridgeError::InvalidTransaction)?;
            sign_protected_transaction_envelope(
                &registry,
                &envelope,
                &unlock_key,
                &expected_public_key,
                &mut transaction,
                &network_passphrase,
            )
            .map_err(classify_protected_signing_error)?;
            let signed_xdr = transaction_envelope_xdr(&transaction)
                .map_err(|_| BridgeError::InvalidTransaction)?;
            Ok(json!({
                "transaction_xdr": STANDARD.encode(signed_xdr),
            }))
        }
        "reveal" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected_public_key = take_string(object, "expected_public_key")?;
            let material = export_signing_material(
                &registry,
                &envelope,
                &passcode,
                &expected_public_key,
            )
            .map_err(classify_protected_signer_error)?;
            match material {
                ExportedSigningMaterial::Secret { secret } => Ok(json!({
                    "kind": "secret",
                    "secret": secret.as_str(),
                })),
                ExportedSigningMaterial::Mnemonic {
                    mnemonic,
                    mnemonic_passphrase,
                    index,
                    language,
                } => Ok(json!({
                    "kind": "mnemonic",
                    "mnemonic": mnemonic.as_str(),
                    "mnemonic_passphrase": mnemonic_passphrase.as_str(),
                    "index": index,
                    "language": language,
                })),
            }
        }
        _ => Err(BridgeError::InvalidRequest("unsupported operation")),
    }
}

fn take_value(object: &mut Map<String, Value>, key: &'static str) -> Result<Value, BridgeError> {
    object
        .remove(key)
        .ok_or(BridgeError::InvalidRequest("missing required field"))
}

fn take_string(object: &mut Map<String, Value>, key: &'static str) -> Result<String, BridgeError> {
    match object.remove(key) {
        Some(Value::String(value)) => Ok(value),
        _ => Err(BridgeError::InvalidField(key)),
    }
}

fn take_sensitive_string(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Zeroizing<String>, BridgeError> {
    take_string(object, key).map(Zeroizing::new)
}

fn take_optional_sensitive_string(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Zeroizing<String>, BridgeError> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(Zeroizing::new(String::new())),
        Some(Value::String(value)) => Ok(Zeroizing::new(value)),
        _ => Err(BridgeError::InvalidField(key)),
    }
}

fn take_optional_string(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Option<String>, BridgeError> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err(BridgeError::InvalidField(key)),
    }
}

fn take_optional_usize(
    object: &mut Map<String, Value>,
    key: &'static str,
    default: usize,
) -> Result<usize, BridgeError> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(BridgeError::InvalidField(key)),
        _ => Err(BridgeError::InvalidField(key)),
    }
}

fn decode_unlock_key(encoded: &str) -> Result<WalletUnlockKey, BridgeError> {
    let bytes = Zeroizing::new(
        STANDARD
            .decode(encoded.as_bytes())
            .map_err(|_| BridgeError::InvalidUnlockKey)?,
    );
    let bytes: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| BridgeError::InvalidUnlockKey)?;
    Ok(WalletUnlockKey::from_bytes(bytes))
}

fn classify_wallet_material_error(error: WalletMaterialError) -> BridgeError {
    match error {
        WalletMaterialError::Signer(_) | WalletMaterialError::Derivation(_) => {
            BridgeError::InvalidWalletMaterial(error.to_string())
        }
        WalletMaterialError::Protection(ProtectionError::SecretStore(
            SecretStoreError::EmptyPassword,
        )) => BridgeError::InvalidPasscode,
        other => BridgeError::Core(other.to_string()),
    }
}

fn classify_protected_signer_error(error: ProtectedSignerError) -> BridgeError {
    match error {
        ProtectedSignerError::Protection(ProtectionError::SecretStore(
            SecretStoreError::InvalidPassword | SecretStoreError::EmptyPassword,
        )) => BridgeError::InvalidPasscode,
        ProtectedSignerError::Protection(ProtectionError::SecretStore(
            SecretStoreError::InvalidUnlockKey,
        )) => BridgeError::InvalidUnlockKey,
        ProtectedSignerError::IdentityMismatch => BridgeError::IdentityMismatch,
        ProtectedSignerError::InvalidExpectedPublicKey => {
            BridgeError::InvalidRequest("expected_public_key is invalid")
        }
        other => BridgeError::Core(other.to_string()),
    }
}

fn classify_protected_signing_error(error: ProtectedSigningError) -> BridgeError {
    match error {
        ProtectedSigningError::Unlock(error) => classify_protected_signer_error(error),
        ProtectedSigningError::Transaction(_) => BridgeError::InvalidTransaction,
    }
}

enum BridgeError {
    InvalidRequest(&'static str),
    InvalidField(&'static str),
    InvalidWalletMaterial(String),
    InvalidPasscode,
    InvalidUnlockKey,
    IdentityMismatch,
    InvalidTransaction,
    Core(String),
}

impl BridgeError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) | Self::InvalidField(_) | Self::InvalidWalletMaterial(_) => {
                "invalid-input"
            }
            Self::InvalidPasscode => "invalid-passcode",
            Self::InvalidUnlockKey => "invalid-unlock-key",
            Self::IdentityMismatch => "identity-mismatch",
            Self::InvalidTransaction => "invalid-transaction",
            Self::Core(_) => "core-error",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidRequest(message) => (*message).to_owned(),
            Self::InvalidField(field) => format!("invalid or missing field: {field}"),
            Self::InvalidWalletMaterial(message) => message.clone(),
            Self::InvalidPasscode => "invalid wallet passcode".to_owned(),
            Self::InvalidUnlockKey => "invalid wallet unlock key".to_owned(),
            Self::IdentityMismatch => "wallet identity does not match metadata".to_owned(),
            Self::InvalidTransaction => "invalid Stellar transaction".to_owned(),
            Self::Core(message) => message.clone(),
        }
    }
}

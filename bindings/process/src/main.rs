use std::io::{self, Read, Write};
use std::process;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_sdk::{FresnicaSdk, SdkError, SdkSigningMaterialKind, SDK_API_VERSION};
use serde_json::{json, Map, Value};
use zeroize::Zeroizing;

pub const PROCESS_BINDING_API_VERSION: u64 = 2;

fn main() {
    match run() {
        Ok(result) => write_response(
            json!({"ok": true, "protocol_version": PROCESS_BINDING_API_VERSION, "result": result}),
            0,
        ),
        Err(error) => write_response(
            json!({
                "ok": false,
                "protocol_version": PROCESS_BINDING_API_VERSION,
                "error": {"code": error.code(), "message": error.message()},
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
    let sdk = FresnicaSdk::new();

    match operation.as_str() {
        "version" => {
            let version = sdk.version();
            Ok(json!({
                "process_binding_version": env!("CARGO_PKG_VERSION"),
                "process_binding_api_version": PROCESS_BINDING_API_VERSION,
                "sdk_api_version": SDK_API_VERSION,
                "client_api_version": version.core_client_api_version,
                "protocol_version": PROCESS_BINDING_API_VERSION,
            }))
        }
        "parse-account" => {
            let address = take_string(object, "address")?;
            let identity = sdk.parse_account(address)?;
            Ok(json!({
                "address": identity.address,
                "kind": match identity.kind {
                    fresnica_sdk::SdkAccountKind::Classic => "classic",
                    fresnica_sdk::SdkAccountKind::Contract => "contract",
                },
                "public_key": identity.public_key,
            }))
        }
        "protect-secret" => {
            let secret = take_sensitive_string(object, "secret")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected = take_optional_string(object, "expected_signer_public_key")?;
            let protected = sdk.protect_secret(
                secret.as_str().to_owned(),
                passcode.as_str().to_owned(),
                expected,
            )?;
            protected_json(protected)
        }
        "protect-mnemonic" => {
            let mnemonic = take_sensitive_string(object, "mnemonic")?;
            let mnemonic_passphrase =
                take_optional_sensitive_string(object, "mnemonic_passphrase")?;
            let index = take_optional_u32(object, "index", 0)?;
            let language = take_optional_string(object, "language")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected = take_optional_string(object, "expected_signer_public_key")?;
            let protected = sdk.protect_mnemonic(
                mnemonic.as_str().to_owned(),
                mnemonic_passphrase.as_str().to_owned(),
                index,
                language,
                passcode.as_str().to_owned(),
                expected,
            )?;
            protected_json(protected)
        }
        "generate-mnemonic" => {
            let language =
                take_optional_string(object, "language")?.unwrap_or_else(|| "english".to_owned());
            let strength = take_optional_u32(object, "strength", 256)?;
            let mnemonic_passphrase =
                take_optional_sensitive_string(object, "mnemonic_passphrase")?;
            let index = take_optional_u32(object, "index", 0)?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let generated = sdk.generate_mnemonic(
                language,
                strength,
                mnemonic_passphrase.as_str().to_owned(),
                index,
                passcode.as_str().to_owned(),
            )?;
            Ok(json!({
                "signer_public_key": generated.signer.signer_public_key,
                "envelope": parse_envelope_json(&generated.signer.envelope_json)?,
                "mnemonic": generated.mnemonic,
                "language": generated.language,
                "index": generated.index,
            }))
        }
        "derive-mnemonic-signer" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            let index = take_optional_u32(object, "index", 0)?;
            let protected = sdk.derive_mnemonic_signer(
                envelope.to_string(),
                passcode.as_str().to_owned(),
                expected,
                index,
            )?;
            protected_json(protected)
        }
        "reprotect" => {
            let envelope = take_value(object, "envelope")?;
            let current_passcode = take_sensitive_string(object, "current_passcode")?;
            let new_passcode = take_sensitive_string(object, "new_passcode")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            let protected = sdk.reprotect(
                envelope.to_string(),
                current_passcode.as_str().to_owned(),
                new_passcode.as_str().to_owned(),
                expected,
            )?;
            protected_json(protected)
        }
        "derive-unlock-key" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            let key = sdk.derive_unlock_key(
                envelope.to_string(),
                passcode.as_str().to_owned(),
                expected,
            )?;
            Ok(json!({"unlock_key": STANDARD.encode(key)}))
        }
        "validate-unlock-key" => {
            let envelope = take_value(object, "envelope")?;
            let unlock_key =
                decode_base64(&take_sensitive_string(object, "unlock_key")?, "unlock_key")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            sdk.validate_unlock_key(envelope.to_string(), unlock_key, expected)?;
            Ok(json!({}))
        }
        "sign-transaction" => {
            let envelope = take_value(object, "envelope")?;
            let unlock_key =
                decode_base64(&take_sensitive_string(object, "unlock_key")?, "unlock_key")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            let transaction_xdr =
                decode_base64(&take_string(object, "transaction_xdr")?, "transaction_xdr")?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let signed = sdk.sign_transaction_xdr(
                envelope.to_string(),
                unlock_key,
                expected,
                transaction_xdr,
                network_passphrase,
            )?;
            Ok(json!({"transaction_xdr": STANDARD.encode(signed)}))
        }
        "sign-transaction-with-passcode" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            let transaction_xdr =
                decode_base64(&take_string(object, "transaction_xdr")?, "transaction_xdr")?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let signed = sdk.sign_transaction_xdr_with_passcode(
                envelope.to_string(),
                passcode.as_str().to_owned(),
                expected,
                transaction_xdr,
                network_passphrase,
            )?;
            Ok(json!({"transaction_xdr": STANDARD.encode(signed)}))
        }
        "sign-soroban-authorization" => {
            let envelope = take_value(object, "envelope")?;
            let unlock_key =
                decode_base64(&take_sensitive_string(object, "unlock_key")?, "unlock_key")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            let authorization_entry_xdr = decode_base64(
                &take_string(object, "authorization_entry_xdr")?,
                "authorization_entry_xdr",
            )?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let signed = sdk.sign_soroban_authorization_xdr(
                envelope.to_string(),
                unlock_key,
                expected,
                authorization_entry_xdr,
                network_passphrase,
            )?;
            Ok(json!({"authorization_entry_xdr": STANDARD.encode(signed)}))
        }
        "sign-soroban-authorization-with-passcode" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            let authorization_entry_xdr = decode_base64(
                &take_string(object, "authorization_entry_xdr")?,
                "authorization_entry_xdr",
            )?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let signed = sdk.sign_soroban_authorization_xdr_with_passcode(
                envelope.to_string(),
                passcode.as_str().to_owned(),
                expected,
                authorization_entry_xdr,
                network_passphrase,
            )?;
            Ok(json!({"authorization_entry_xdr": STANDARD.encode(signed)}))
        }
        "reveal" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected = take_string(object, "expected_signer_public_key")?;
            let material =
                sdk.reveal(envelope.to_string(), passcode.as_str().to_owned(), expected)?;
            Ok(match material.kind {
                SdkSigningMaterialKind::Secret => json!({
                    "kind": "secret",
                    "secret": material.secret,
                }),
                SdkSigningMaterialKind::Mnemonic => json!({
                    "kind": "mnemonic",
                    "mnemonic": material.mnemonic,
                    "mnemonic_passphrase": material.mnemonic_passphrase,
                    "index": material.index,
                    "language": material.language,
                }),
            })
        }
        "prepare-ed25519-signing" => {
            let transaction_xdr =
                decode_base64(&take_string(object, "transaction_xdr")?, "transaction_xdr")?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let prepared = sdk.prepare_ed25519_signing(transaction_xdr, network_passphrase)?;
            Ok(json!({
                "transaction_hash": STANDARD.encode(prepared.transaction_hash),
                "transaction_xdr": STANDARD.encode(prepared.transaction_xdr),
                "network_passphrase": prepared.network_passphrase,
            }))
        }
        "apply-ed25519-signature" => {
            let transaction_xdr =
                decode_base64(&take_string(object, "transaction_xdr")?, "transaction_xdr")?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let signer_public_key = take_string(object, "signer_public_key")?;
            let signature = decode_base64(&take_string(object, "signature")?, "signature")?;
            let signed = sdk.apply_ed25519_signature(
                transaction_xdr,
                network_passphrase,
                signer_public_key,
                signature,
            )?;
            Ok(json!({"transaction_xdr": STANDARD.encode(signed)}))
        }
        "prepare-soroban-authorization-signing" => {
            let authorization_entry_xdr = decode_base64(
                &take_string(object, "authorization_entry_xdr")?,
                "authorization_entry_xdr",
            )?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let prepared = sdk.prepare_soroban_authorization_signing(
                authorization_entry_xdr,
                network_passphrase,
            )?;
            Ok(json!({
                "authorization_hash": STANDARD.encode(prepared.authorization_hash),
                "authorization_entry_xdr": STANDARD.encode(prepared.authorization_entry_xdr),
                "authorization_preimage_xdr": STANDARD.encode(prepared.authorization_preimage_xdr),
                "network_passphrase": prepared.network_passphrase,
            }))
        }
        "apply-soroban-ed25519-signature" => {
            let authorization_entry_xdr = decode_base64(
                &take_string(object, "authorization_entry_xdr")?,
                "authorization_entry_xdr",
            )?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let signer_public_key = take_string(object, "signer_public_key")?;
            let signature = decode_base64(&take_string(object, "signature")?, "signature")?;
            let signed = sdk.apply_soroban_ed25519_signature(
                authorization_entry_xdr,
                network_passphrase,
                signer_public_key,
                signature,
            )?;
            Ok(json!({"authorization_entry_xdr": STANDARD.encode(signed)}))
        }
        _ => Err(BridgeError::InvalidRequest("unsupported operation")),
    }
}

fn protected_json(
    protected: fresnica_sdk::SdkProtectedSoftwareSigner,
) -> Result<Value, BridgeError> {
    Ok(json!({
        "signer_public_key": protected.signer_public_key,
        "envelope": parse_envelope_json(&protected.envelope_json)?,
    }))
}

fn parse_envelope_json(encoded: &str) -> Result<Value, BridgeError> {
    serde_json::from_str(encoded)
        .map_err(|_| BridgeError::InvalidRequest("SDK returned invalid envelope JSON"))
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

fn take_optional_u32(
    object: &mut Map<String, Value>,
    key: &'static str,
    default: u32,
) -> Result<u32, BridgeError> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(BridgeError::InvalidField(key)),
        _ => Err(BridgeError::InvalidField(key)),
    }
}

fn decode_base64(encoded: &str, field: &'static str) -> Result<Vec<u8>, BridgeError> {
    STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| BridgeError::InvalidField(field))
}

enum BridgeError {
    InvalidRequest(&'static str),
    InvalidField(&'static str),
    Sdk(SdkError),
}

impl From<SdkError> for BridgeError {
    fn from(error: SdkError) -> Self {
        Self::Sdk(error)
    }
}

impl BridgeError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) | Self::InvalidField(_) => "invalid-input",
            Self::Sdk(error) => error.code.as_str(),
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidRequest(message) => (*message).to_owned(),
            Self::InvalidField(field) => format!("invalid or missing field: {field}"),
            Self::Sdk(error) => error.message.clone(),
        }
    }
}

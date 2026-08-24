use std::io::{self, Read, Write};
use std::process;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_core::{
    ClientApiError, CoreClientApi, ExportedSigningMaterial, WalletUnlockKey, CLIENT_API_VERSION,
};
use serde_json::{json, Map, Value};
use zeroize::Zeroizing;

const PROTOCOL_VERSION: u64 = 2;

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
    let api = CoreClientApi::new();

    match operation.as_str() {
        "version" => Ok(json!({
            "core_version": env!("CARGO_PKG_VERSION"),
            "client_api_version": CLIENT_API_VERSION,
            "protocol_version": PROTOCOL_VERSION,
        })),
        "parse-account" => {
            let address = take_string(object, "address")?;
            let identity = api.parse_account(&address)?;
            Ok(json!({
                "address": identity.address,
                "kind": identity.kind.as_str(),
                "public_key": identity.public_key,
            }))
        }
        "protect-secret" => {
            let secret = take_sensitive_string(object, "secret")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected_signer_public_key =
                take_optional_string(object, "expected_signer_public_key")?;
            let protected = api.protect_secret(
                &secret,
                &passcode,
                expected_signer_public_key.as_deref(),
            )?;
            Ok(json!({
                "signer_public_key": protected.signer_public_key,
                "envelope": protected.envelope,
            }))
        }
        "protect-mnemonic" => {
            let mnemonic = take_sensitive_string(object, "mnemonic")?;
            let mnemonic_passphrase =
                take_optional_sensitive_string(object, "mnemonic_passphrase")?;
            let index = take_optional_usize(object, "index", 0)?;
            let language = take_optional_string(object, "language")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected_signer_public_key =
                take_optional_string(object, "expected_signer_public_key")?;
            let protected = api.protect_mnemonic(
                &mnemonic,
                &mnemonic_passphrase,
                index,
                language.as_deref(),
                &passcode,
                expected_signer_public_key.as_deref(),
            )?;
            Ok(json!({
                "signer_public_key": protected.signer_public_key,
                "envelope": protected.envelope,
            }))
        }
        "generate-mnemonic" => {
            let language = take_optional_string(object, "language")?
                .unwrap_or_else(|| "english".to_owned());
            let strength = take_optional_usize(object, "strength", 256)?;
            let index = take_optional_usize(object, "index", 0)?;
            let mnemonic_passphrase =
                take_optional_sensitive_string(object, "mnemonic_passphrase")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let generated = api.generate_mnemonic(
                &language,
                strength,
                &mnemonic_passphrase,
                index,
                &passcode,
            )?;
            Ok(json!({
                "signer_public_key": generated.signer.signer_public_key,
                "envelope": generated.signer.envelope,
                "mnemonic": generated.mnemonic.as_str(),
                "language": generated.language,
                "index": generated.index,
            }))
        }
        "reprotect" => {
            let envelope = take_value(object, "envelope")?;
            let current_passcode = take_sensitive_string(object, "current_passcode")?;
            let new_passcode = take_sensitive_string(object, "new_passcode")?;
            let expected_signer_public_key = take_string(object, "expected_signer_public_key")?;
            let protected = api.reprotect(
                &envelope,
                &current_passcode,
                &new_passcode,
                &expected_signer_public_key,
            )?;
            Ok(json!({
                "signer_public_key": protected.signer_public_key,
                "envelope": protected.envelope,
            }))
        }
        "derive-unlock-key" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected_signer_public_key = take_string(object, "expected_signer_public_key")?;
            let unlock_key = api.derive_unlock_key(
                &envelope,
                &passcode,
                &expected_signer_public_key,
            )?;
            Ok(json!({
                "unlock_key": STANDARD.encode(unlock_key.as_bytes()),
            }))
        }
        "validate-unlock-key" => {
            let envelope = take_value(object, "envelope")?;
            let unlock_key = decode_unlock_key(&take_sensitive_string(object, "unlock_key")?)?;
            let expected_signer_public_key = take_string(object, "expected_signer_public_key")?;
            api.validate_unlock_key(
                &envelope,
                &unlock_key,
                &expected_signer_public_key,
            )?;
            Ok(json!({}))
        }
        "sign-transaction" => {
            let envelope = take_value(object, "envelope")?;
            let unlock_key = decode_unlock_key(&take_sensitive_string(object, "unlock_key")?)?;
            let expected_signer_public_key = take_string(object, "expected_signer_public_key")?;
            let transaction_xdr = take_string(object, "transaction_xdr")?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let raw_xdr = decode_base64(&transaction_xdr, "transaction_xdr")?;
            let signed_xdr = api.sign_transaction_xdr(
                &envelope,
                &unlock_key,
                &expected_signer_public_key,
                &raw_xdr,
                &network_passphrase,
            )?;
            Ok(json!({
                "transaction_xdr": STANDARD.encode(signed_xdr),
            }))
        }
        "reveal" => {
            let envelope = take_value(object, "envelope")?;
            let passcode = take_sensitive_string(object, "passcode")?;
            let expected_signer_public_key = take_string(object, "expected_signer_public_key")?;
            let material = api.reveal(
                &envelope,
                &passcode,
                &expected_signer_public_key,
            )?;
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
        "prepare-ed25519-signing" => {
            let transaction_xdr = take_string(object, "transaction_xdr")?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let raw_xdr = decode_base64(&transaction_xdr, "transaction_xdr")?;
            let prepared = api.prepare_ed25519_signing(&raw_xdr, &network_passphrase)?;
            Ok(json!({
                "transaction_hash": STANDARD.encode(prepared.transaction_hash),
                "transaction_xdr": STANDARD.encode(prepared.transaction_xdr),
                "network_passphrase": prepared.network_passphrase,
            }))
        }
        "apply-ed25519-signature" => {
            let transaction_xdr = take_string(object, "transaction_xdr")?;
            let network_passphrase = take_string(object, "network_passphrase")?;
            let signer_public_key = take_string(object, "signer_public_key")?;
            let signature = take_string(object, "signature")?;
            let raw_xdr = decode_base64(&transaction_xdr, "transaction_xdr")?;
            let raw_signature = decode_base64(&signature, "signature")?;
            let signed_xdr = api.apply_ed25519_signature(
                &raw_xdr,
                &network_passphrase,
                &signer_public_key,
                &raw_signature,
            )?;
            Ok(json!({
                "transaction_xdr": STANDARD.encode(signed_xdr),
            }))
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

fn decode_base64(encoded: &str, field: &'static str) -> Result<Vec<u8>, BridgeError> {
    STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| BridgeError::InvalidField(field))
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

enum BridgeError {
    InvalidRequest(&'static str),
    InvalidField(&'static str),
    InvalidUnlockKey,
    Client(ClientApiError),
}

impl From<ClientApiError> for BridgeError {
    fn from(error: ClientApiError) -> Self {
        Self::Client(error)
    }
}

impl BridgeError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) | Self::InvalidField(_) => "invalid-input",
            Self::InvalidUnlockKey => "invalid-unlock-key",
            Self::Client(error) => error.code().as_str(),
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidRequest(message) => (*message).to_owned(),
            Self::InvalidField(field) => format!("invalid or missing field: {field}"),
            Self::InvalidUnlockKey => "invalid wallet unlock key".to_owned(),
            Self::Client(error) => error.message().to_owned(),
        }
    }
}

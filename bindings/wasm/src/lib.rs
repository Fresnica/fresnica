//! Browser/WASM binding over the platform-neutral `fresnica-sdk`.
//!
//! This boundary intentionally does not export raw `WalletUnlockKey` material.
//! Browser routine signing takes a fresh application passcode and keeps the
//! derived unlock key inside Rust for the entire signing operation.

use fresnica_sdk::{
    FresnicaSdk, SdkAccountIdentity, SdkAccountKind, SdkEd25519SigningRequest, SdkError,
    SdkExportedSigningMaterial, SdkGeneratedMnemonic, SdkProtectedSoftwareSigner,
    SdkSigningMaterialKind,
};
use js_sys::{Error as JsError, Reflect, Uint8Array};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Version of the browser/WASM binding surface.
pub const WASM_BINDING_API_VERSION: u64 = 1;

#[wasm_bindgen]
pub struct FresnicaWasmSdk {
    sdk: FresnicaSdk,
}

impl Default for FresnicaWasmSdk {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl FresnicaWasmSdk {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            sdk: FresnicaSdk::new(),
        }
    }

    #[wasm_bindgen(js_name = version)]
    pub fn version(&self) -> Result<JsValue, JsValue> {
        let version = self.sdk.version();
        to_js(&WasmSdkVersion {
            wasm_binding_api_version: WASM_BINDING_API_VERSION,
            sdk_api_version: version.sdk_api_version,
            core_client_api_version: version.core_client_api_version,
        })
    }

    #[wasm_bindgen(js_name = parseAccount)]
    pub fn parse_account(&self, address: String) -> Result<JsValue, JsValue> {
        self.sdk
            .parse_account(address)
            .map(WasmAccountIdentity::from)
            .map_err(sdk_error_to_js)
            .and_then(|identity| to_js(&identity))
    }

    #[wasm_bindgen(js_name = protectSecret)]
    pub fn protect_secret(
        &self,
        secret: String,
        passcode: String,
        expected_signer_public_key: Option<String>,
    ) -> Result<JsValue, JsValue> {
        self.sdk
            .protect_secret(secret, passcode, expected_signer_public_key)
            .map(WasmProtectedSoftwareSigner::from)
            .map_err(sdk_error_to_js)
            .and_then(|signer| to_js(&signer))
    }

    #[wasm_bindgen(js_name = protectMnemonic)]
    pub fn protect_mnemonic(
        &self,
        mnemonic: String,
        mnemonic_passphrase: String,
        index: u32,
        language: Option<String>,
        passcode: String,
        expected_signer_public_key: Option<String>,
    ) -> Result<JsValue, JsValue> {
        self.sdk
            .protect_mnemonic(
                mnemonic,
                mnemonic_passphrase,
                index,
                language,
                passcode,
                expected_signer_public_key,
            )
            .map(WasmProtectedSoftwareSigner::from)
            .map_err(sdk_error_to_js)
            .and_then(|signer| to_js(&signer))
    }

    #[wasm_bindgen(js_name = generateMnemonic)]
    pub fn generate_mnemonic(
        &self,
        language: String,
        strength: u32,
        mnemonic_passphrase: String,
        index: u32,
        passcode: String,
    ) -> Result<JsValue, JsValue> {
        self.sdk
            .generate_mnemonic(language, strength, mnemonic_passphrase, index, passcode)
            .map(WasmGeneratedMnemonic::from)
            .map_err(sdk_error_to_js)
            .and_then(|generated| to_js(&generated))
    }

    #[wasm_bindgen(js_name = reprotect)]
    pub fn reprotect(
        &self,
        envelope_json: String,
        current_passcode: String,
        new_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<JsValue, JsValue> {
        self.sdk
            .reprotect(
                envelope_json,
                current_passcode,
                new_passcode,
                expected_signer_public_key,
            )
            .map(WasmProtectedSoftwareSigner::from)
            .map_err(sdk_error_to_js)
            .and_then(|signer| to_js(&signer))
    }

    /// Browser routine signing. The derived unlock key never crosses the WASM
    /// boundary and is dropped/zeroized inside Rust after signing.
    #[wasm_bindgen(js_name = signTransactionXdrWithPasscode)]
    pub fn sign_transaction_xdr_with_passcode(
        &self,
        envelope_json: String,
        passcode: String,
        expected_signer_public_key: String,
        transaction_xdr: Uint8Array,
        network_passphrase: String,
    ) -> Result<Uint8Array, JsValue> {
        self.sdk
            .sign_transaction_xdr_with_passcode(
                envelope_json,
                passcode,
                expected_signer_public_key,
                transaction_xdr.to_vec(),
                network_passphrase,
            )
            .map(|signed| Uint8Array::from(signed.as_slice()))
            .map_err(sdk_error_to_js)
    }

    /// Explicit recovery-material declassification. The caller must provide a
    /// fresh application passcode for each reveal operation.
    #[wasm_bindgen(js_name = reveal)]
    pub fn reveal(
        &self,
        envelope_json: String,
        fresh_passcode: String,
        expected_signer_public_key: String,
    ) -> Result<JsValue, JsValue> {
        self.sdk
            .reveal(envelope_json, fresh_passcode, expected_signer_public_key)
            .map(WasmExportedSigningMaterial::from)
            .map_err(sdk_error_to_js)
            .and_then(|material| to_js(&material))
    }

    #[wasm_bindgen(js_name = prepareEd25519Signing)]
    pub fn prepare_ed25519_signing(
        &self,
        transaction_xdr: Uint8Array,
        network_passphrase: String,
    ) -> Result<JsValue, JsValue> {
        self.sdk
            .prepare_ed25519_signing(transaction_xdr.to_vec(), network_passphrase)
            .map(WasmEd25519SigningRequest::from)
            .map_err(sdk_error_to_js)
            .and_then(|request| to_js(&request))
    }

    #[wasm_bindgen(js_name = applyEd25519Signature)]
    pub fn apply_ed25519_signature(
        &self,
        transaction_xdr: Uint8Array,
        network_passphrase: String,
        signer_public_key: String,
        signature: Uint8Array,
    ) -> Result<Uint8Array, JsValue> {
        self.sdk
            .apply_ed25519_signature(
                transaction_xdr.to_vec(),
                network_passphrase,
                signer_public_key,
                signature.to_vec(),
            )
            .map(|signed| Uint8Array::from(signed.as_slice()))
            .map_err(sdk_error_to_js)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmSdkVersion {
    wasm_binding_api_version: u64,
    sdk_api_version: u64,
    core_client_api_version: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmAccountIdentity {
    kind: &'static str,
    address: String,
    public_key: Option<String>,
}

impl From<SdkAccountIdentity> for WasmAccountIdentity {
    fn from(identity: SdkAccountIdentity) -> Self {
        Self {
            kind: match identity.kind {
                SdkAccountKind::Classic => "classic",
                SdkAccountKind::Contract => "contract",
            },
            address: identity.address,
            public_key: identity.public_key,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmProtectedSoftwareSigner {
    signer_public_key: String,
    envelope_json: String,
}

impl From<SdkProtectedSoftwareSigner> for WasmProtectedSoftwareSigner {
    fn from(signer: SdkProtectedSoftwareSigner) -> Self {
        Self {
            signer_public_key: signer.signer_public_key,
            envelope_json: signer.envelope_json,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmGeneratedMnemonic {
    signer: WasmProtectedSoftwareSigner,
    mnemonic: String,
    language: String,
    index: u32,
}

impl From<SdkGeneratedMnemonic> for WasmGeneratedMnemonic {
    fn from(generated: SdkGeneratedMnemonic) -> Self {
        Self {
            signer: generated.signer.into(),
            mnemonic: generated.mnemonic,
            language: generated.language,
            index: generated.index,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmEd25519SigningRequest {
    #[serde(with = "serde_bytes")]
    transaction_hash: Vec<u8>,
    #[serde(with = "serde_bytes")]
    transaction_xdr: Vec<u8>,
    network_passphrase: String,
}

impl From<SdkEd25519SigningRequest> for WasmEd25519SigningRequest {
    fn from(request: SdkEd25519SigningRequest) -> Self {
        Self {
            transaction_hash: request.transaction_hash,
            transaction_xdr: request.transaction_xdr,
            network_passphrase: request.network_passphrase,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmExportedSigningMaterial {
    kind: &'static str,
    secret: Option<String>,
    mnemonic: Option<String>,
    mnemonic_passphrase: Option<String>,
    index: Option<u32>,
    language: Option<String>,
}

impl From<SdkExportedSigningMaterial> for WasmExportedSigningMaterial {
    fn from(material: SdkExportedSigningMaterial) -> Self {
        Self {
            kind: match material.kind {
                SdkSigningMaterialKind::Secret => "secret",
                SdkSigningMaterialKind::Mnemonic => "mnemonic",
            },
            secret: material.secret,
            mnemonic: material.mnemonic,
            mnemonic_passphrase: material.mnemonic_passphrase,
            index: material.index,
            language: material.language,
        }
    }
}

fn to_js<T: Serialize + ?Sized>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|error| {
        new_js_error(
            "core-error",
            &format!("unable to serialize Fresnica WASM result: {error}"),
        )
    })
}

fn sdk_error_to_js(error: SdkError) -> JsValue {
    new_js_error(error.code.as_str(), &error.message)
}

fn new_js_error(code: &str, message: &str) -> JsValue {
    let error: JsValue = JsError::new(message).into();
    let _ = Reflect::set(
        &error,
        &JsValue::from_str("name"),
        &JsValue::from_str("FresnicaSdkError"),
    );
    let _ = Reflect::set(&error, &JsValue::from_str("code"), &JsValue::from_str(code));
    error
}

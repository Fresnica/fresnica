use std::collections::BTreeMap;
use std::io::Read;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_sdk::{FresnicaSdk, SdkAccountKind};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use stellar_xdr::{
    AccountId, Memo, MuxedAccount, OperationBody, Preconditions, PublicKey, TimeBounds,
    TransactionEnvelope,
};
use toml::Value as TomlValue;
use ureq::unversioned::multipart::Form;
use url::Url;
use zeroize::Zeroizing;

use crate::payment::PaymentMemo;
use crate::service::FresnicaClient;
use crate::transaction::{
    has_valid_transaction_signature, network_passphrase, parse_transaction_xdr,
};

const MAX_ANCHOR_DOCUMENT_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorTransferKind {
    Deposit,
    Withdraw,
}

impl AnchorTransferKind {
    pub fn endpoint(self) -> &'static str {
        match self {
            Self::Deposit => "deposit",
            Self::Withdraw => "withdraw",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorProtocol {
    Sep24,
    Sep6,
}

impl AnchorProtocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sep24 => "SEP-24",
            Self::Sep6 => "SEP-6",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnchorAsset {
    pub code: String,
    pub issuer: String,
}

impl AnchorAsset {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (code, issuer) = value
            .trim()
            .split_once(':')
            .ok_or_else(|| "anchor asset must be CODE:GISSUER".to_owned())?;
        let code = code.trim();
        let issuer = issuer.trim();
        if code.is_empty()
            || code.len() > 12
            || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err("Asset code must be 1 to 12 ASCII alphanumeric characters".to_owned());
        }
        let identity = FresnicaSdk::new()
            .parse_account(issuer.to_owned())
            .map_err(|_| "invalid Stellar asset issuer".to_owned())?;
        if identity.kind != SdkAccountKind::Classic {
            return Err("asset issuer must be a Classic G address".to_owned());
        }
        Ok(Self {
            code: code.to_owned(),
            issuer: identity.address,
        })
    }

    pub fn display(&self) -> String {
        format!("{}:{}", self.code, self.issuer)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnchorCapabilities {
    pub domain: String,
    pub sep6_url: Option<String>,
    pub sep24_url: Option<String>,
    pub web_auth_url: Option<String>,
    pub web_auth_for_contracts_url: Option<String>,
    pub signing_key: Option<String>,
    pub web_auth_contract_id: Option<String>,
    pub sep10_auth: bool,
    pub sep45_auth: bool,
    pub kyc_url: Option<String>,
    pub direct_payment_url: Option<String>,
    pub sep6_deposit: bool,
    pub sep6_withdraw: bool,
    pub sep6_deposit_info: JsonValue,
    pub sep6_withdraw_info: JsonValue,
    pub sep6_transaction_info: JsonValue,
    pub sep24_deposit: bool,
    pub sep24_withdraw: bool,
    pub warnings: Vec<String>,
}

impl AnchorCapabilities {
    pub fn customer_server(&self) -> Option<&str> {
        self.kyc_url.as_deref().or(self.sep6_url.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnchorDiscovery {
    pub asset: AnchorAsset,
    pub home_domain: String,
    pub capabilities: AnchorCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorSep10Challenge {
    transaction_xdr: Vec<u8>,
    web_auth_endpoint: String,
    server_signing_key: String,
}

impl AnchorSep10Challenge {
    pub fn transaction_xdr(&self) -> &[u8] {
        &self.transaction_xdr
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnchorSep24InteractiveResult {
    pub url: String,
    pub transaction_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorWithdrawalPayment {
    pub destination: String,
    pub amount: String,
    pub memo: PaymentMemo,
}

fn sep6_transaction_enabled(capabilities: &AnchorCapabilities) -> bool {
    capabilities
        .sep6_transaction_info
        .get("enabled")
        .and_then(JsonValue::as_bool)
        != Some(false)
}

fn sep6_transaction_requires_auth(capabilities: &AnchorCapabilities) -> bool {
    capabilities
        .sep6_transaction_info
        .get("authentication_required")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false)
}

pub fn select_anchor_status_protocol(
    capabilities: &AnchorCapabilities,
    requested: Option<AnchorProtocol>,
) -> Result<AnchorProtocol, String> {
    if let Some(protocol) = requested {
        return validate_status_protocol(capabilities, protocol);
    }

    let sep24_candidate = capabilities.sep24_url.is_some()
        && capabilities.sep10_auth
        && (capabilities.sep24_deposit || capabilities.sep24_withdraw);
    let sep6_candidate = capabilities.sep6_url.is_some()
        && sep6_transaction_enabled(capabilities)
        && (capabilities.sep6_deposit || capabilities.sep6_withdraw)
        && (!sep6_transaction_requires_auth(capabilities) || capabilities.sep10_auth);

    match (sep24_candidate, sep6_candidate) {
        (true, false) => Ok(AnchorProtocol::Sep24),
        (false, true) => Ok(AnchorProtocol::Sep6),
        (true, true) => Err(
            "anchor transaction status is available through both SEP-24 and SEP-6; choose a protocol explicitly"
                .to_owned(),
        ),
        (false, false) => {
            if capabilities.sep24_url.is_some()
                && !capabilities.sep10_auth
                && capabilities.sep45_auth
            {
                return Err("SEP-24 status is available only through SEP-45; SEP-45 contract-account authentication is required".to_owned());
            }
            if capabilities.sep6_url.is_some()
                && sep6_transaction_enabled(capabilities)
                && sep6_transaction_requires_auth(capabilities)
                && !capabilities.sep10_auth
            {
                if capabilities.sep45_auth {
                    return Err("SEP-6 transaction status requires authentication but only SEP-45 is available; SEP-45 contract-account authentication is required".to_owned());
                }
                return Err(
                    "SEP-6 transaction status requires a complete Classic SEP-10 authentication path"
                        .to_owned(),
                );
            }
            Err("anchor does not advertise a uniquely usable SEP-24/SEP-6 transaction-status path; choose a protocol explicitly for an existing transaction".to_owned())
        }
    }
}

fn validate_status_protocol(
    capabilities: &AnchorCapabilities,
    protocol: AnchorProtocol,
) -> Result<AnchorProtocol, String> {
    match protocol {
        AnchorProtocol::Sep24 if capabilities.sep24_url.is_none() => {
            Err("anchor does not advertise a SEP-24 transfer server".to_owned())
        }
        AnchorProtocol::Sep24 if !capabilities.sep10_auth => {
            if capabilities.sep45_auth {
                Err("SEP-24 status is available only through SEP-45; SEP-45 contract-account authentication is required".to_owned())
            } else {
                Err(
                    "SEP-24 status requires a complete Classic SEP-10 authentication path"
                        .to_owned(),
                )
            }
        }
        AnchorProtocol::Sep6 if capabilities.sep6_url.is_none() => {
            Err("anchor does not advertise a SEP-6 transfer server".to_owned())
        }
        AnchorProtocol::Sep6 if !sep6_transaction_enabled(capabilities) => {
            Err("anchor reports that SEP-6 /transaction is disabled".to_owned())
        }
        AnchorProtocol::Sep6
            if sep6_transaction_requires_auth(capabilities) && !capabilities.sep10_auth =>
        {
            if capabilities.sep45_auth {
                Err("SEP-6 transaction status requires authentication but only SEP-45 is available; SEP-45 contract-account authentication is required".to_owned())
            } else {
                Err("SEP-6 transaction status requires a complete Classic SEP-10 authentication path".to_owned())
            }
        }
        _ => Ok(protocol),
    }
}

pub fn anchor_status_requires_sep10(
    capabilities: &AnchorCapabilities,
    protocol: AnchorProtocol,
) -> bool {
    match protocol {
        AnchorProtocol::Sep24 => true,
        AnchorProtocol::Sep6 => sep6_transaction_requires_auth(capabilities),
    }
}

fn parse_anchor_transaction_response(
    value: &JsonValue,
    expected_id: &str,
) -> Result<JsonValue, String> {
    let transaction = value
        .get("transaction")
        .filter(|value| value.is_object())
        .ok_or_else(|| "anchor /transaction response has no transaction object".to_owned())?;
    let id = anchor_transaction_text(transaction, "id")
        .ok_or_else(|| "anchor transaction has no id".to_owned())?;
    if id != expected_id {
        return Err(format!(
            "anchor /transaction response id mismatch: expected {expected_id}, received {id}"
        ));
    }
    if anchor_transaction_text(transaction, "status").is_none() {
        return Err("anchor transaction has no status".to_owned());
    }
    Ok(transaction.clone())
}

pub fn anchor_withdrawal_payment_from_transaction(
    transaction: &JsonValue,
    expected_source: &str,
    asset: &AnchorAsset,
) -> Result<AnchorWithdrawalPayment, String> {
    let status = anchor_transaction_text(transaction, "status")
        .ok_or_else(|| "anchor transaction has no status".to_owned())?;
    if status != "pending_user_transfer_start" {
        return Err(format!(
            "anchor withdrawal is not ready for Stellar payment; current status is {status}"
        ));
    }
    let kind = anchor_transaction_text(transaction, "kind")
        .ok_or_else(|| "anchor transaction has no kind".to_owned())?;
    if !matches!(kind, "withdrawal" | "withdraw") {
        return Err(format!(
            "anchor transaction is {kind}, not a withdrawal payment"
        ));
    }

    let source = anchor_transaction_text(transaction, "from").ok_or_else(|| {
        "anchor withdrawal has no source account; refusing automatic payment".to_owned()
    })?;
    if source != expected_source {
        return Err(format!(
            "anchor withdrawal source account mismatch: expected {expected_source}, received {source}"
        ));
    }
    if let Some(amount_in_asset) = anchor_transaction_text(transaction, "amount_in_asset") {
        let expected_asset = format!("stellar:{}:{}", asset.code, asset.issuer);
        if amount_in_asset != expected_asset {
            return Err(format!(
                "anchor withdrawal input asset mismatch: expected {expected_asset}, received {amount_in_asset}"
            ));
        }
    }

    let destination = anchor_transaction_text(transaction, "withdraw_anchor_account")
        .ok_or_else(|| "anchor withdrawal has no withdraw_anchor_account".to_owned())?;
    let identity = FresnicaSdk::new()
        .parse_account(destination.to_owned())
        .map_err(|_| "anchor withdrawal account is not a valid Stellar address".to_owned())?;
    if identity.kind != SdkAccountKind::Classic {
        return Err("anchor withdrawal account must be a Classic G address".to_owned());
    }
    let amount = anchor_transaction_text(transaction, "amount_in")
        .ok_or_else(|| "anchor withdrawal has no amount_in".to_owned())?;
    let memo_type = anchor_transaction_text(transaction, "withdraw_memo_type");
    let memo = anchor_transaction_text(transaction, "withdraw_memo");

    Ok(AnchorWithdrawalPayment {
        destination: identity.address,
        amount: amount.to_owned(),
        memo: PaymentMemo::from_anchor_fields(memo_type, memo)?,
    })
}

pub fn anchor_transaction_text<'a>(transaction: &'a JsonValue, key: &str) -> Option<&'a str> {
    transaction
        .get(key)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn sep6_info(capabilities: &AnchorCapabilities, kind: AnchorTransferKind) -> &JsonValue {
    match kind {
        AnchorTransferKind::Deposit => &capabilities.sep6_deposit_info,
        AnchorTransferKind::Withdraw => &capabilities.sep6_withdraw_info,
    }
}

pub fn anchor_sep6_requires_auth(
    capabilities: &AnchorCapabilities,
    kind: AnchorTransferKind,
) -> bool {
    sep6_info(capabilities, kind)
        .get("authentication_required")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false)
}

pub fn select_anchor_transfer_protocol(
    capabilities: &AnchorCapabilities,
    kind: AnchorTransferKind,
) -> Result<AnchorProtocol, String> {
    let sep24_enabled = match kind {
        AnchorTransferKind::Deposit => capabilities.sep24_deposit,
        AnchorTransferKind::Withdraw => capabilities.sep24_withdraw,
    };
    if sep24_enabled && capabilities.sep24_url.is_some() && capabilities.sep10_auth {
        return Ok(AnchorProtocol::Sep24);
    }

    let sep6_enabled = match kind {
        AnchorTransferKind::Deposit => capabilities.sep6_deposit,
        AnchorTransferKind::Withdraw => capabilities.sep6_withdraw,
    };
    if sep6_enabled && capabilities.sep6_url.is_some() {
        if anchor_sep6_requires_auth(capabilities, kind) && !capabilities.sep10_auth {
            return Err(format!(
                "SEP-6 {} requires authentication, but no complete Classic SEP-10 path is available",
                kind.endpoint()
            ));
        }
        return Ok(AnchorProtocol::Sep6);
    }

    if sep24_enabled && capabilities.sep24_url.is_some() {
        if capabilities.sep45_auth {
            return Err(format!(
                "SEP-24 {} is available only through SEP-45; SEP-45 contract-account authentication is required",
                kind.endpoint()
            ));
        }
        return Err(format!(
            "SEP-24 {} requires a complete Classic SEP-10 authentication path",
            kind.endpoint()
        ));
    }

    Err(format!(
        "No usable SEP-24/SEP-6 {} flow is advertised for this asset",
        kind.endpoint()
    ))
}

pub fn anchor_transfer_requires_sep10(
    capabilities: &AnchorCapabilities,
    protocol: AnchorProtocol,
    kind: AnchorTransferKind,
) -> bool {
    match protocol {
        AnchorProtocol::Sep24 => true,
        AnchorProtocol::Sep6 => anchor_sep6_requires_auth(capabilities, kind),
    }
}

pub fn start_anchor_sep24_transfer(
    account: &str,
    asset: &AnchorAsset,
    capabilities: &AnchorCapabilities,
    kind: AnchorTransferKind,
    fields: &BTreeMap<String, String>,
    token: &str,
) -> Result<AnchorSep24InteractiveResult, String> {
    if token.trim().is_empty() {
        return Err("SEP-24 requires a non-empty SEP-10 token".to_owned());
    }
    let base = capabilities
        .sep24_url
        .as_deref()
        .ok_or_else(|| format!("SEP-24 {} is not available", kind.endpoint()))?;
    let authorization = Zeroizing::new(format!("Bearer {token}"));
    let endpoint = format!(
        "{}/transactions/{}/interactive",
        base.trim_end_matches('/'),
        kind.endpoint()
    );

    let mut form = Form::new()
        .text("asset_code", asset.code.as_str())
        .text("asset_issuer", asset.issuer.as_str())
        .text("account", account);
    for (name, value) in fields {
        form = form.text(name.as_str(), value.as_str());
    }

    let request = ureq::post(&endpoint)
        .header("Authorization", authorization.as_str())
        .config()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    let mut response = request
        .send(form)
        .map_err(|error| format!("Unable to call SEP-24 endpoint {endpoint}: {error}"))?;
    let status = response.status().as_u16();
    reject_anchor_redirect(status, &endpoint)?;
    let value = response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_DOCUMENT_BYTES)
        .read_json::<JsonValue>()
        .map_err(|error| format!("Invalid JSON from SEP-24 endpoint {endpoint}: {error}"))?;
    if !(200..300).contains(&status) {
        return Err(anchor_http_error("SEP-24", &endpoint, status, &value));
    }
    parse_sep24_interactive_response(&value)
}

fn parse_sep24_interactive_response(
    value: &JsonValue,
) -> Result<AnchorSep24InteractiveResult, String> {
    let response_type = value
        .get("type")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    if response_type != "interactive_customer_info_needed" {
        return Err("Anchor returned an invalid SEP-24 interactive response type".to_owned());
    }
    let url = value
        .get("url")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Anchor SEP-24 interactive response has no URL".to_owned())?;
    let parsed = Url::parse(url)
        .map_err(|_| "Anchor SEP-24 interactive response URL is invalid".to_owned())?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(
            "Anchor SEP-24 interactive response URL must be HTTPS without embedded credentials"
                .to_owned(),
        );
    }
    let transaction_id = value
        .get("id")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Anchor SEP-24 interactive response has no transaction id".to_owned())?;
    Ok(AnchorSep24InteractiveResult {
        url: url.to_owned(),
        transaction_id: transaction_id.to_owned(),
    })
}

pub fn start_anchor_sep6_transfer(
    account: &str,
    asset: &AnchorAsset,
    capabilities: &AnchorCapabilities,
    kind: AnchorTransferKind,
    fields: &BTreeMap<String, String>,
    token: Option<&str>,
) -> Result<JsonValue, String> {
    let base = capabilities
        .sep6_url
        .as_deref()
        .ok_or_else(|| format!("SEP-6 {} is not available", kind.endpoint()))?;
    let token = token.filter(|token| !token.trim().is_empty());
    if anchor_sep6_requires_auth(capabilities, kind) && token.is_none() {
        return Err(format!(
            "SEP-6 {} requires SEP-10 authentication",
            kind.endpoint()
        ));
    }
    let fields = sep6_request_fields(capabilities, kind, fields)?;
    let url = sep6_request_url(base, kind, asset, account, &fields)?;
    let authorization = token.map(|token| Zeroizing::new(format!("Bearer {token}")));

    let mut request = ureq::get(url.as_str());
    if let Some(authorization) = authorization.as_ref() {
        request = request.header("Authorization", authorization.as_str());
    }
    let endpoint = endpoint_label(&url);
    let request = request
        .config()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    let mut response = request
        .call()
        .map_err(|error| format!("Unable to call SEP-6 endpoint {endpoint}: {error}"))?;
    let status = response.status().as_u16();
    reject_anchor_redirect(status, &endpoint)?;
    let value = response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_DOCUMENT_BYTES)
        .read_json::<JsonValue>()
        .map_err(|error| format!("Invalid JSON from SEP-6 endpoint {endpoint}: {error}"))?;
    if !(200..300).contains(&status) && status != 403 {
        return Err(anchor_http_error("SEP-6", &endpoint, status, &value));
    }
    if !value.is_object() {
        return Err(format!(
            "Anchor SEP-6 response from {endpoint} is malformed"
        ));
    }
    Ok(value)
}

fn sep6_request_fields(
    capabilities: &AnchorCapabilities,
    kind: AnchorTransferKind,
    fields: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut fields = fields.clone();
    if fields.contains_key("funding_method") {
        return Ok(fields);
    }
    let Some(methods) = sep6_info(capabilities, kind)
        .get("funding_methods")
        .and_then(JsonValue::as_array)
    else {
        return Ok(fields);
    };
    let methods = methods
        .iter()
        .filter_map(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    match methods.as_slice() {
        [method] => {
            fields.insert("funding_method".to_owned(), (*method).to_owned());
            Ok(fields)
        }
        [] => Ok(fields),
        _ => Err(format!(
            "SEP-6 {} requires an explicit funding_method because multiple methods are advertised",
            kind.endpoint()
        )),
    }
}

fn sep6_request_url(
    base: &str,
    kind: AnchorTransferKind,
    asset: &AnchorAsset,
    account: &str,
    fields: &BTreeMap<String, String>,
) -> Result<Url, String> {
    let endpoint = format!("{}/{}", base.trim_end_matches('/'), kind.endpoint());
    let mut url = Url::parse(&endpoint)
        .map_err(|_| format!("SEP-6 {} endpoint is invalid", kind.endpoint()))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("asset_code", &asset.code);
        query.append_pair("account", account);
        for (name, value) in fields {
            query.append_pair(name, value);
        }
    }
    Ok(url)
}

fn anchor_http_error(protocol: &str, endpoint: &str, status: u16, value: &JsonValue) -> String {
    let detail = value
        .get("error")
        .and_then(JsonValue::as_str)
        .or_else(|| value.get("message").and_then(JsonValue::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match detail {
        Some(detail) => format!("{protocol} endpoint {endpoint} returned HTTP {status}: {detail}"),
        None => format!("{protocol} endpoint {endpoint} returned HTTP {status}"),
    }
}

pub fn fetch_anchor_transaction(
    capabilities: &AnchorCapabilities,
    protocol: AnchorProtocol,
    transaction_id: &str,
    token: Option<&str>,
) -> Result<JsonValue, String> {
    let transaction_id = transaction_id.trim();
    if transaction_id.is_empty() {
        return Err("anchor transaction id must not be empty".to_owned());
    }
    let base = match protocol {
        AnchorProtocol::Sep24 => capabilities
            .sep24_url
            .as_deref()
            .ok_or_else(|| "SEP-24 transfer server is unavailable".to_owned())?,
        AnchorProtocol::Sep6 => capabilities
            .sep6_url
            .as_deref()
            .ok_or_else(|| "SEP-6 transfer server is unavailable".to_owned())?,
    };
    let token = token.filter(|token| !token.trim().is_empty());
    if anchor_status_requires_sep10(capabilities, protocol) && token.is_none() {
        return Err(format!(
            "{} transaction status requires SEP-10 authentication",
            protocol.label()
        ));
    }

    let mut url = Url::parse(&format!("{}/transaction", base.trim_end_matches('/')))
        .map_err(|_| format!("{} transaction endpoint is invalid", protocol.label()))?;
    url.query_pairs_mut().append_pair("id", transaction_id);
    let authorization = token.map(|token| Zeroizing::new(format!("Bearer {token}")));
    let mut request = ureq::get(url.as_str());
    if let Some(authorization) = authorization.as_ref() {
        request = request.header("Authorization", authorization.as_str());
    }
    let endpoint = endpoint_label(&url);
    let request = request
        .config()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    let mut response = request.call().map_err(|error| {
        format!(
            "Unable to call {} endpoint {endpoint}: {error}",
            protocol.label()
        )
    })?;
    let status = response.status().as_u16();
    reject_anchor_redirect(status, &endpoint)?;
    let value = response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_DOCUMENT_BYTES)
        .read_json::<JsonValue>()
        .map_err(|error| {
            format!(
                "Invalid JSON from {} endpoint {endpoint}: {error}",
                protocol.label()
            )
        })?;
    if status == 404 {
        return Err(format!(
            "{} transaction {transaction_id} was not found",
            protocol.label()
        ));
    }
    if !(200..300).contains(&status) {
        return Err(anchor_http_error(
            protocol.label(),
            &endpoint,
            status,
            &value,
        ));
    }
    parse_anchor_transaction_response(&value, transaction_id)
}

impl FresnicaClient {
    pub fn discover_anchor(&self, asset_text: &str) -> Result<AnchorDiscovery, String> {
        let asset = AnchorAsset::parse(asset_text)?;
        let issuer = self.horizon().get_account(&asset.issuer)?;
        let home_domain = issuer
            .get("home_domain")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("Asset issuer {} has no home_domain", asset.issuer))?;
        let home_domain = valid_domain(home_domain)?;
        let capabilities = discover(&asset, &home_domain)?;
        Ok(AnchorDiscovery {
            asset,
            home_domain,
            capabilities,
        })
    }
}

fn discover(asset: &AnchorAsset, home_domain: &str) -> Result<AnchorCapabilities, String> {
    let domain = valid_domain(home_domain)?;
    let toml_url = format!("https://{domain}/.well-known/stellar.toml");
    let document = fetch_text(&toml_url, "stellar.toml")?;
    capabilities_from_document(asset, &domain, &document, fetch_json)
}

fn capabilities_from_document<F>(
    asset: &AnchorAsset,
    domain: &str,
    document: &str,
    mut load_json: F,
) -> Result<AnchorCapabilities, String>
where
    F: FnMut(&str) -> Result<JsonValue, String>,
{
    let document: TomlValue = toml::from_str(document)
        .map_err(|error| format!("Issuer stellar.toml is invalid: {error}"))?;
    if !currency_matches(&document, asset) {
        return Ok(AnchorCapabilities {
            domain: domain.to_owned(),
            sep6_url: None,
            sep24_url: None,
            web_auth_url: None,
            web_auth_for_contracts_url: None,
            signing_key: None,
            web_auth_contract_id: None,
            sep10_auth: false,
            sep45_auth: false,
            kyc_url: None,
            direct_payment_url: None,
            sep6_deposit: false,
            sep6_withdraw: false,
            sep6_deposit_info: serde_json::json!({}),
            sep6_withdraw_info: serde_json::json!({}),
            sep6_transaction_info: serde_json::json!({}),
            sep24_deposit: false,
            sep24_withdraw: false,
            warnings: vec!["stellar.toml does not list this exact asset".to_owned()],
        });
    }

    let sep6_url = endpoint(&document, "TRANSFER_SERVER")?;
    let sep24_url = endpoint(&document, "TRANSFER_SERVER_SEP0024")?;
    let web_auth_url = endpoint(&document, "WEB_AUTH_ENDPOINT")?;
    let web_auth_for_contracts_url = endpoint(&document, "WEB_AUTH_FOR_CONTRACTS_ENDPOINT")?;
    let signing_key = account_identifier(&document, "SIGNING_KEY", SdkAccountKind::Classic)?;
    let web_auth_contract_id =
        account_identifier(&document, "WEB_AUTH_CONTRACT_ID", SdkAccountKind::Contract)?;
    let kyc_url = endpoint(&document, "KYC_SERVER")?;
    let direct_payment_url = endpoint(&document, "DIRECT_PAYMENT_SERVER")?;
    let mut warnings = Vec::new();
    let mut sep6_deposit = false;
    let mut sep6_withdraw = false;
    let mut sep6_deposit_info = serde_json::json!({});
    let mut sep6_withdraw_info = serde_json::json!({});
    let mut sep6_transaction_info = serde_json::json!({});
    let mut sep24_deposit = false;
    let mut sep24_withdraw = false;

    if let Some(base) = &sep6_url {
        match load_json(&info_url(base)) {
            Ok(info) => {
                sep6_deposit_info = asset_info(info.get("deposit"), &asset.code)
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                sep6_withdraw_info = asset_info(info.get("withdraw"), &asset.code)
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                sep6_transaction_info = info
                    .get("transaction")
                    .filter(|value| value.is_object())
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                sep6_deposit = asset_enabled(info.get("deposit"), &asset.code);
                sep6_withdraw = asset_enabled(info.get("withdraw"), &asset.code);
            }
            Err(error) => warnings.push(format!("SEP-6 /info unavailable: {error}")),
        }
    }

    if let Some(base) = &sep24_url {
        match load_json(&info_url(base)) {
            Ok(info) => {
                sep24_deposit = asset_enabled(info.get("deposit"), &asset.code);
                sep24_withdraw = asset_enabled(info.get("withdraw"), &asset.code);
            }
            Err(error) => warnings.push(format!("SEP-24 /info unavailable: {error}")),
        }
    }

    let sep10_ready = web_auth_url.is_some() && signing_key.is_some();
    let sep45_ready = web_auth_for_contracts_url.is_some()
        && web_auth_contract_id.is_some()
        && signing_key.is_some();
    let sep6_requires_auth = [
        &sep6_deposit_info,
        &sep6_withdraw_info,
        &sep6_transaction_info,
    ]
    .into_iter()
    .any(|info| {
        info.get("authentication_required")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false)
    });

    if sep6_requires_auth && !sep10_ready && !sep45_ready {
        warnings.push(
            "SEP-6 authentication is required but no complete SEP-10/SEP-45 metadata is available"
                .to_owned(),
        );
    }
    if (sep24_deposit || sep24_withdraw) && !sep10_ready && !sep45_ready {
        warnings.push(
            "SEP-24 is advertised but no complete SEP-10/SEP-45 authentication metadata is available"
                .to_owned(),
        );
    }

    Ok(AnchorCapabilities {
        domain: domain.to_owned(),
        sep6_url,
        sep24_url,
        web_auth_url,
        web_auth_for_contracts_url,
        signing_key,
        web_auth_contract_id,
        sep10_auth: sep10_ready,
        sep45_auth: sep45_ready,
        kyc_url,
        direct_payment_url,
        sep6_deposit,
        sep6_withdraw,
        sep6_deposit_info,
        sep6_withdraw_info,
        sep6_transaction_info,
        sep24_deposit,
        sep24_withdraw,
        warnings,
    })
}

#[derive(Debug, Deserialize)]
struct Sep10ChallengeResponse {
    transaction: String,
    #[serde(default)]
    network_passphrase: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Sep10TokenResponse {
    token: String,
}

pub fn prepare_anchor_sep10_challenge(
    network: &str,
    account: &str,
    home_domain: &str,
    capabilities: &AnchorCapabilities,
) -> Result<AnchorSep10Challenge, String> {
    let web_auth_endpoint = capabilities.web_auth_url.as_deref().ok_or_else(|| {
        format!(
            "{} does not advertise a SEP-10 WEB_AUTH_ENDPOINT",
            capabilities.domain
        )
    })?;
    let signing_key = capabilities.signing_key.as_deref().ok_or_else(|| {
        format!(
            "{} does not advertise a SEP-10 SIGNING_KEY",
            capabilities.domain
        )
    })?;
    let challenge = request_sep10_challenge(web_auth_endpoint, account)?;
    validate_sep10_network_passphrase(network, challenge.network_passphrase.as_deref())?;
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before Unix epoch".to_owned())?
        .as_secs();
    let transaction_xdr = verify_sep10_challenge(
        &challenge.transaction,
        network,
        account,
        signing_key,
        home_domain,
        web_auth_endpoint,
        now_unix,
    )?;
    Ok(AnchorSep10Challenge {
        transaction_xdr,
        web_auth_endpoint: web_auth_endpoint.to_owned(),
        server_signing_key: signing_key.to_owned(),
    })
}

pub fn exchange_anchor_sep10_challenge(
    network: &str,
    account: &str,
    challenge: &AnchorSep10Challenge,
    signed_xdr: Vec<u8>,
) -> Result<Zeroizing<String>, String> {
    let verified_envelope = parse_transaction_xdr(&challenge.transaction_xdr)?;
    let signed_envelope = parse_transaction_xdr(&signed_xdr)?;
    let (TransactionEnvelope::Tx(verified), TransactionEnvelope::Tx(signed)) =
        (&verified_envelope, &signed_envelope)
    else {
        return Err("SEP-10 challenge must use a classic transaction envelope".to_owned());
    };
    if signed.tx != verified.tx {
        return Err("signed SEP-10 transaction does not match the verified challenge".to_owned());
    }
    if !has_valid_transaction_signature(&signed_envelope, network, &challenge.server_signing_key)? {
        return Err(
            "signed SEP-10 challenge no longer contains the verified server signature".to_owned(),
        );
    }
    if !has_valid_transaction_signature(&signed_envelope, network, account)? {
        return Err("signed SEP-10 challenge does not contain a valid client signature".to_owned());
    }
    exchange_sep10_challenge(&challenge.web_auth_endpoint, &STANDARD.encode(signed_xdr))
}

fn validate_sep10_network_passphrase(
    network: &str,
    server_network_passphrase: Option<&str>,
) -> Result<(), String> {
    if let Some(server_network_passphrase) = server_network_passphrase {
        if server_network_passphrase != network_passphrase(network)? {
            return Err(format!(
                "SEP-10 server network_passphrase does not match local {network} configuration"
            ));
        }
    }
    Ok(())
}

fn request_sep10_challenge(
    web_auth_endpoint: &str,
    account: &str,
) -> Result<Sep10ChallengeResponse, String> {
    let url = sep10_challenge_url(web_auth_endpoint, account)?;
    let value = fetch_json(url.as_str())?;
    serde_json::from_value(value).map_err(|error| {
        format!("Invalid SEP-10 challenge response from {web_auth_endpoint}: {error}")
    })
}

fn sep10_challenge_url(web_auth_endpoint: &str, account: &str) -> Result<Url, String> {
    let mut url = Url::parse(web_auth_endpoint)
        .map_err(|_| "WEB_AUTH_ENDPOINT must be a valid URL".to_owned())?;
    url.query_pairs_mut().append_pair("account", account);
    Ok(url)
}

fn exchange_sep10_challenge(
    web_auth_endpoint: &str,
    signed_transaction: &str,
) -> Result<Zeroizing<String>, String> {
    let mut response = ureq::post(web_auth_endpoint)
        .config()
        .https_only(true)
        .max_redirects(0)
        .build()
        .send_json(serde_json::json!({"transaction": signed_transaction}))
        .map_err(|error| {
            format!("Unable to exchange SEP-10 challenge at {web_auth_endpoint}: {error}")
        })?;
    reject_anchor_redirect(response.status().as_u16(), web_auth_endpoint)?;
    let value = response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_DOCUMENT_BYTES)
        .read_json::<Sep10TokenResponse>()
        .map_err(|error| {
            format!("Invalid SEP-10 token response from {web_auth_endpoint}: {error}")
        })?;
    if value.token.trim().is_empty() {
        return Err("SEP-10 token response did not include a token".to_owned());
    }
    Ok(Zeroizing::new(value.token))
}

fn verify_sep10_challenge(
    challenge_xdr: &str,
    network: &str,
    client_account: &str,
    server_signing_key: &str,
    home_domain: &str,
    web_auth_endpoint: &str,
    now_unix: u64,
) -> Result<Vec<u8>, String> {
    let transaction_xdr = STANDARD
        .decode(challenge_xdr.trim())
        .map_err(|_| "SEP-10 challenge transaction is not valid base64 XDR".to_owned())?;
    let envelope = parse_transaction_xdr(&transaction_xdr)?;
    let TransactionEnvelope::Tx(transaction_envelope) = &envelope else {
        return Err("SEP-10 challenge must use a classic transaction envelope".to_owned());
    };
    let transaction = &transaction_envelope.tx;
    let server_account = classic_muxed_account(server_signing_key, "SEP-10 SIGNING_KEY")?;
    let client_account = classic_muxed_account(client_account, "SEP-10 client account")?;

    if transaction.source_account != server_account {
        return Err("SEP-10 challenge source account does not match SIGNING_KEY".to_owned());
    }
    if transaction.seq_num.0 != 0 {
        return Err("SEP-10 challenge sequence number must be zero".to_owned());
    }
    if transaction.memo != Memo::None {
        return Err("SEP-10 challenge contains an unexpected memo".to_owned());
    }

    let time_bounds = transaction_time_bounds(&transaction.cond)?;
    if time_bounds.max_time.0 == 0
        || time_bounds.min_time.0 > time_bounds.max_time.0
        || now_unix < time_bounds.min_time.0
        || now_unix > time_bounds.max_time.0
    {
        return Err("SEP-10 challenge is outside its valid time bounds".to_owned());
    }

    let first = transaction
        .operations
        .first()
        .ok_or_else(|| "SEP-10 challenge has no operations".to_owned())?;
    if first.source_account.as_ref() != Some(&client_account) {
        return Err("SEP-10 first operation source does not match the client account".to_owned());
    }
    let OperationBody::ManageData(first_data) = &first.body else {
        return Err("SEP-10 first operation must be ManageData".to_owned());
    };
    let expected_key = format!("{} auth", valid_domain(home_domain)?);
    if xdr_string64(&first_data.data_name)? != expected_key {
        return Err("SEP-10 first operation home-domain key is invalid".to_owned());
    }
    let nonce = first_data
        .data_value
        .as_ref()
        .ok_or_else(|| "SEP-10 challenge nonce is missing".to_owned())?;
    if AsRef::<[u8]>::as_ref(nonce).len() != 64 {
        return Err("SEP-10 challenge nonce must be exactly 64 bytes".to_owned());
    }

    let expected_web_auth_domain = web_auth_domain(web_auth_endpoint)?;
    let mut web_auth_domain_seen = false;
    for operation in transaction.operations.iter().skip(1) {
        let OperationBody::ManageData(data) = &operation.body else {
            return Err("SEP-10 additional operations must be ManageData".to_owned());
        };
        let key = xdr_string64(&data.data_name)?;
        if key == "client_domain" {
            return Err(
                "SEP-10 challenge contains an unexpected client_domain operation".to_owned(),
            );
        }
        if operation.source_account.as_ref() != Some(&server_account) {
            return Err("SEP-10 additional operation source must be SIGNING_KEY".to_owned());
        }
        if key == "web_auth_domain" {
            if web_auth_domain_seen {
                return Err(
                    "SEP-10 challenge contains duplicate web_auth_domain operations".to_owned(),
                );
            }
            let value = data
                .data_value
                .as_ref()
                .ok_or_else(|| "SEP-10 web_auth_domain value is missing".to_owned())?;
            let value = std::str::from_utf8(AsRef::<[u8]>::as_ref(value))
                .map_err(|_| "SEP-10 web_auth_domain is not UTF-8".to_owned())?;
            if value != expected_web_auth_domain {
                return Err("SEP-10 web_auth_domain does not match WEB_AUTH_ENDPOINT".to_owned());
            }
            web_auth_domain_seen = true;
        }
    }
    if !web_auth_domain_seen {
        return Err("SEP-10 challenge is missing web_auth_domain".to_owned());
    }

    if !has_valid_transaction_signature(&envelope, network, server_signing_key)? {
        return Err("SEP-10 challenge is not signed by SIGNING_KEY".to_owned());
    }

    Ok(transaction_xdr)
}

fn transaction_time_bounds(preconditions: &Preconditions) -> Result<&TimeBounds, String> {
    match preconditions {
        Preconditions::Time(bounds) => Ok(bounds),
        Preconditions::V2(value) => value
            .time_bounds
            .as_ref()
            .ok_or_else(|| "SEP-10 challenge must include time bounds".to_owned()),
        Preconditions::None => Err("SEP-10 challenge must include time bounds".to_owned()),
    }
}

fn classic_muxed_account(address: &str, label: &str) -> Result<MuxedAccount, String> {
    let account =
        AccountId::from_str(address).map_err(|_| format!("{label} must be a Classic G address"))?;
    match account.0 {
        PublicKey::PublicKeyTypeEd25519(key) => Ok(MuxedAccount::Ed25519(key)),
    }
}

fn xdr_string64(value: &stellar_xdr::String64) -> Result<String, String> {
    std::str::from_utf8(AsRef::<[u8]>::as_ref(value))
        .map(str::to_owned)
        .map_err(|_| "SEP-10 ManageData key is not UTF-8".to_owned())
}

fn web_auth_domain(endpoint: &str) -> Result<String, String> {
    let url =
        Url::parse(endpoint).map_err(|_| "WEB_AUTH_ENDPOINT must be a valid URL".to_owned())?;
    let host = url
        .host_str()
        .ok_or_else(|| "WEB_AUTH_ENDPOINT must include a host".to_owned())?;
    Ok(host.to_owned())
}

fn valid_domain(value: &str) -> Result<String, String> {
    let domain = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty()
        || domain.contains("://")
        || domain.contains('/')
        || domain.contains('\\')
        || domain.chars().any(char::is_whitespace)
    {
        return Err("Issuer home_domain is not a valid host name".to_owned());
    }
    Ok(domain)
}

fn endpoint(document: &TomlValue, key: &str) -> Result<Option<String>, String> {
    let Some(value) = text(document, key) else {
        return Ok(None);
    };
    let parsed = Url::parse(&value).map_err(|_| format!("{key} must be a valid HTTPS URL"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(format!(
            "{key} must be an HTTPS URL without embedded credentials"
        ));
    }
    Ok(Some(value))
}

fn account_identifier(
    document: &TomlValue,
    key: &str,
    expected_kind: SdkAccountKind,
) -> Result<Option<String>, String> {
    let Some(value) = text(document, key) else {
        return Ok(None);
    };
    let identity = FresnicaSdk::new()
        .parse_account(value)
        .map_err(|_| format!("{key} must be a valid Stellar account identifier"))?;
    if identity.kind != expected_kind {
        let kind = match expected_kind {
            SdkAccountKind::Classic => "Classic G address",
            SdkAccountKind::Contract => "Contract C address",
        };
        return Err(format!("{key} must be a {kind}"));
    }
    Ok(Some(identity.address))
}

fn text(document: &TomlValue, key: &str) -> Option<String> {
    document
        .get(key)
        .and_then(TomlValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn currency_matches(document: &TomlValue, asset: &AnchorAsset) -> bool {
    let Some(currencies) = document.get("CURRENCIES").and_then(TomlValue::as_array) else {
        return true;
    };
    currencies.iter().any(|currency| {
        let Some(table) = currency.as_table() else {
            return false;
        };
        let code_matches = table
            .get("code")
            .and_then(TomlValue::as_str)
            .is_some_and(|code| code == asset.code);
        if !code_matches {
            return false;
        }
        table
            .get("issuer")
            .and_then(TomlValue::as_str)
            .is_some_and(|issuer| issuer == asset.issuer)
    })
}

fn asset_info<'a>(section: Option<&'a JsonValue>, code: &str) -> Option<&'a JsonValue> {
    let section = section?.as_object()?;
    let value = section.get(code)?;
    value.as_object()?;
    Some(value)
}

fn asset_enabled(section: Option<&JsonValue>, code: &str) -> bool {
    asset_info(section, code)
        .and_then(|value| value.get("enabled"))
        .and_then(JsonValue::as_bool)
        .unwrap_or_else(|| asset_info(section, code).is_some())
}

fn fetch_text(url: &str, label: &str) -> Result<String, String> {
    let mut response = ureq::get(url)
        .config()
        .https_only(true)
        .max_redirects(0)
        .build()
        .call()
        .map_err(|error| format!("Unable to load {label} from {url}: {error}"))?;
    reject_anchor_redirect(response.status().as_u16(), url)?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_DOCUMENT_BYTES)
        .read_to_string()
        .map_err(|error| format!("Unable to read {label} from {url}: {error}"))
}

fn fetch_json(url: &str) -> Result<JsonValue, String> {
    let mut response = ureq::get(url)
        .config()
        .https_only(true)
        .max_redirects(0)
        .build()
        .call()
        .map_err(|error| format!("Unable to load {url}: {error}"))?;
    reject_anchor_redirect(response.status().as_u16(), url)?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_DOCUMENT_BYTES)
        .read_json::<JsonValue>()
        .map_err(|error| format!("Invalid JSON from {url}: {error}"))
}

fn info_url(base: &str) -> String {
    format!("{}/info", base.trim_end_matches('/'))
}

fn endpoint_label(url: &Url) -> String {
    let mut sanitized = url.clone();
    sanitized.set_query(None);
    sanitized.set_fragment(None);
    sanitized.to_string()
}

fn reject_anchor_redirect(status: u16, endpoint: &str) -> Result<(), String> {
    if (300..400).contains(&status) {
        return Err(format!(
            "Anchor endpoint {endpoint} returned HTTP {status}; redirects are not allowed"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use stellar_xdr::{
        DataValue, Limits, ManageDataOp, Operation, SequenceNumber, String64, TimePoint,
        Transaction, TransactionExt, TransactionV1Envelope, VecM, WriteXdr,
    };

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
    const SERVER_SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const SERVER_PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const TESTNET: &str = "testnet";
    const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";
    const HOME_DOMAIN: &str = "anchor.example";
    const WEB_AUTH_ENDPOINT: &str = "https://auth.example.com/sep10";
    const NOW: u64 = 1_800_000_000;

    fn asset() -> AnchorAsset {
        AnchorAsset::parse(&format!("USD:{ISSUER}")).unwrap()
    }

    fn transfer_capabilities() -> AnchorCapabilities {
        AnchorCapabilities {
            domain: HOME_DOMAIN.to_owned(),
            sep6_url: Some("https://anchor.example/sep6".to_owned()),
            sep24_url: Some("https://anchor.example/sep24".to_owned()),
            web_auth_url: Some(WEB_AUTH_ENDPOINT.to_owned()),
            web_auth_for_contracts_url: None,
            signing_key: Some(SERVER_PUBLIC.to_owned()),
            web_auth_contract_id: None,
            sep10_auth: true,
            sep45_auth: false,
            kyc_url: None,
            direct_payment_url: None,
            sep6_deposit: true,
            sep6_withdraw: true,
            sep6_deposit_info: serde_json::json!({
                "enabled": true,
                "authentication_required": false,
                "funding_methods": ["WIRE"]
            }),
            sep6_withdraw_info: serde_json::json!({
                "enabled": true,
                "authentication_required": true,
                "funding_methods": ["WIRE"]
            }),
            sep6_transaction_info: serde_json::json!({
                "enabled": true,
                "authentication_required": true
            }),
            sep24_deposit: true,
            sep24_withdraw: true,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn sep10_challenge_request_binds_account_without_optional_extensions() {
        let url = sep10_challenge_url(WEB_AUTH_ENDPOINT, ISSUER).unwrap();
        let pairs = url.query_pairs().collect::<Vec<_>>();

        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("auth.example.com"));
        assert!(pairs
            .iter()
            .any(|(key, value)| key == "account" && value == ISSUER));
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn sep10_server_network_is_consistency_check_not_authority() {
        assert!(validate_sep10_network_passphrase(TESTNET, None).is_ok());
        assert!(validate_sep10_network_passphrase(TESTNET, Some(TESTNET_PASSPHRASE)).is_ok());
        assert_eq!(
            validate_sep10_network_passphrase(
                TESTNET,
                Some("Public Global Stellar Network ; September 2015"),
            )
            .unwrap_err(),
            "SEP-10 server network_passphrase does not match local testnet configuration"
        );
    }

    #[test]
    fn sep10_web_auth_domain_is_hostname_not_transport_port() {
        assert_eq!(
            web_auth_domain("https://auth.example.com:8443/sep10").unwrap(),
            "auth.example.com"
        );
    }

    fn manage_data(source: &str, key: &str, value: Vec<u8>) -> Operation {
        Operation {
            source_account: Some(classic_muxed_account(source, "test source").unwrap()),
            body: OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(key.as_bytes().to_vec()).unwrap(),
                data_value: Some(DataValue::try_from(value).unwrap()),
            }),
        }
    }

    fn challenge_envelope(
        client: &str,
        home_domain: &str,
        web_auth_domain: &str,
        min_time: u64,
        max_time: u64,
        nonce_len: usize,
    ) -> TransactionEnvelope {
        let operations: VecM<Operation, 100> = vec![
            manage_data(
                client,
                &format!("{home_domain} auth"),
                vec![b'n'; nonce_len],
            ),
            manage_data(
                SERVER_PUBLIC,
                "web_auth_domain",
                web_auth_domain.as_bytes().to_vec(),
            ),
        ]
        .try_into()
        .unwrap();
        TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: Transaction {
                source_account: classic_muxed_account(SERVER_PUBLIC, "server").unwrap(),
                fee: 200,
                seq_num: SequenceNumber(0),
                cond: Preconditions::Time(TimeBounds {
                    min_time: TimePoint(min_time),
                    max_time: TimePoint(max_time),
                }),
                memo: Memo::None,
                operations,
                ext: TransactionExt::V0,
            },
            signatures: VecM::default(),
        })
    }

    fn encode_envelope(envelope: &TransactionEnvelope) -> String {
        STANDARD.encode(envelope.to_xdr(Limits::none()).unwrap())
    }

    fn signed_valid_challenge() -> &'static str {
        static CHALLENGE: OnceLock<String> = OnceLock::new();
        CHALLENGE.get_or_init(|| {
            let envelope = challenge_envelope(
                ISSUER,
                HOME_DOMAIN,
                "auth.example.com",
                NOW - 30,
                NOW + 300,
                64,
            );
            let unsigned = envelope.to_xdr(Limits::none()).unwrap();
            let sdk = FresnicaSdk::new();
            let protected = sdk
                .protect_secret(
                    SERVER_SECRET.to_owned(),
                    "test-passcode".to_owned(),
                    Some(SERVER_PUBLIC.to_owned()),
                )
                .unwrap();
            let signed = sdk
                .sign_transaction_xdr_with_passcode(
                    protected.envelope_json,
                    "test-passcode".to_owned(),
                    SERVER_PUBLIC.to_owned(),
                    unsigned,
                    TESTNET_PASSPHRASE.to_owned(),
                )
                .unwrap();
            STANDARD.encode(signed)
        })
    }

    #[test]
    fn verifies_signed_sep10_challenge() {
        let verified = verify_sep10_challenge(
            signed_valid_challenge(),
            TESTNET,
            ISSUER,
            SERVER_PUBLIC,
            HOME_DOMAIN,
            WEB_AUTH_ENDPOINT,
            NOW,
        )
        .unwrap();

        assert!(!verified.is_empty());
    }

    #[test]
    fn sep10_exchange_rejects_transaction_substitution_before_http() {
        let transaction_xdr = verify_sep10_challenge(
            signed_valid_challenge(),
            TESTNET,
            ISSUER,
            SERVER_PUBLIC,
            HOME_DOMAIN,
            WEB_AUTH_ENDPOINT,
            NOW,
        )
        .unwrap();
        let challenge = AnchorSep10Challenge {
            transaction_xdr,
            web_auth_endpoint: WEB_AUTH_ENDPOINT.to_owned(),
            server_signing_key: SERVER_PUBLIC.to_owned(),
        };
        let substituted = challenge_envelope(
            ISSUER,
            HOME_DOMAIN,
            "auth.example.com",
            NOW - 30,
            NOW + 301,
            64,
        )
        .to_xdr(Limits::none())
        .unwrap();

        assert_eq!(
            exchange_anchor_sep10_challenge(TESTNET, ISSUER, &challenge, substituted).unwrap_err(),
            "signed SEP-10 transaction does not match the verified challenge"
        );
    }

    #[test]
    fn sep10_exchange_requires_client_signature_before_http() {
        let transaction_xdr = verify_sep10_challenge(
            signed_valid_challenge(),
            TESTNET,
            ISSUER,
            SERVER_PUBLIC,
            HOME_DOMAIN,
            WEB_AUTH_ENDPOINT,
            NOW,
        )
        .unwrap();
        let challenge = AnchorSep10Challenge {
            transaction_xdr: transaction_xdr.clone(),
            web_auth_endpoint: WEB_AUTH_ENDPOINT.to_owned(),
            server_signing_key: SERVER_PUBLIC.to_owned(),
        };

        assert_eq!(
            exchange_anchor_sep10_challenge(TESTNET, ISSUER, &challenge, transaction_xdr)
                .unwrap_err(),
            "signed SEP-10 challenge does not contain a valid client signature"
        );
    }

    #[test]
    fn sep10_verifier_binds_server_signature_to_network() {
        let error = verify_sep10_challenge(
            signed_valid_challenge(),
            "mainnet",
            ISSUER,
            SERVER_PUBLIC,
            HOME_DOMAIN,
            WEB_AUTH_ENDPOINT,
            NOW,
        )
        .unwrap_err();

        assert_eq!(error, "SEP-10 challenge is not signed by SIGNING_KEY");
    }

    #[test]
    fn sep10_verifier_rejects_expired_or_malformed_challenge() {
        let expired = challenge_envelope(
            ISSUER,
            HOME_DOMAIN,
            "auth.example.com",
            NOW - 300,
            NOW - 1,
            64,
        );
        assert_eq!(
            verify_sep10_challenge(
                &encode_envelope(&expired),
                TESTNET,
                ISSUER,
                SERVER_PUBLIC,
                HOME_DOMAIN,
                WEB_AUTH_ENDPOINT,
                NOW,
            )
            .unwrap_err(),
            "SEP-10 challenge is outside its valid time bounds"
        );

        let short_nonce = challenge_envelope(
            ISSUER,
            HOME_DOMAIN,
            "auth.example.com",
            NOW - 30,
            NOW + 300,
            63,
        );
        assert_eq!(
            verify_sep10_challenge(
                &encode_envelope(&short_nonce),
                TESTNET,
                ISSUER,
                SERVER_PUBLIC,
                HOME_DOMAIN,
                WEB_AUTH_ENDPOINT,
                NOW,
            )
            .unwrap_err(),
            "SEP-10 challenge nonce must be exactly 64 bytes"
        );
    }

    #[test]
    fn sep10_verifier_binds_home_and_web_auth_domains() {
        let wrong_home = challenge_envelope(
            ISSUER,
            "other.example",
            "auth.example.com",
            NOW - 30,
            NOW + 300,
            64,
        );
        assert_eq!(
            verify_sep10_challenge(
                &encode_envelope(&wrong_home),
                TESTNET,
                ISSUER,
                SERVER_PUBLIC,
                HOME_DOMAIN,
                WEB_AUTH_ENDPOINT,
                NOW,
            )
            .unwrap_err(),
            "SEP-10 first operation home-domain key is invalid"
        );

        let wrong_auth_domain =
            challenge_envelope(ISSUER, HOME_DOMAIN, "evil.example", NOW - 30, NOW + 300, 64);
        assert_eq!(
            verify_sep10_challenge(
                &encode_envelope(&wrong_auth_domain),
                TESTNET,
                ISSUER,
                SERVER_PUBLIC,
                HOME_DOMAIN,
                WEB_AUTH_ENDPOINT,
                NOW,
            )
            .unwrap_err(),
            "SEP-10 web_auth_domain does not match WEB_AUTH_ENDPOINT"
        );
    }

    #[test]
    fn sep10_verifier_rejects_unsigned_challenge() {
        let unsigned = challenge_envelope(
            ISSUER,
            HOME_DOMAIN,
            "auth.example.com",
            NOW - 30,
            NOW + 300,
            64,
        );
        assert_eq!(
            verify_sep10_challenge(
                &encode_envelope(&unsigned),
                TESTNET,
                ISSUER,
                SERVER_PUBLIC,
                HOME_DOMAIN,
                WEB_AUTH_ENDPOINT,
                NOW,
            )
            .unwrap_err(),
            "SEP-10 challenge is not signed by SIGNING_KEY"
        );
    }

    #[test]
    fn issued_asset_requires_full_identity() {
        assert!(AnchorAsset::parse("USD").is_err());
        assert!(AnchorAsset::parse("XLM").is_err());
        assert_eq!(asset().display(), format!("USD:{ISSUER}"));
    }

    #[test]
    fn discovery_matches_exact_currency_and_reads_sep_info() {
        let document = format!(
            r#"TRANSFER_SERVER = "https://anchor.example/sep6"
TRANSFER_SERVER_SEP0024 = "https://anchor.example/sep24"
WEB_AUTH_ENDPOINT = "https://anchor.example/auth"
WEB_AUTH_FOR_CONTRACTS_ENDPOINT = "https://anchor.example/sep45/auth"
SIGNING_KEY = "{ISSUER}"
WEB_AUTH_CONTRACT_ID = "{CONTRACT}"

[[CURRENCIES]]
code = "USD"
issuer = "{ISSUER}"
"#
        );
        let capabilities =
            capabilities_from_document(&asset(), "anchor.example", &document, |url| match url {
                "https://anchor.example/sep6/info" => Ok(serde_json::json!({
                    "deposit": {"USD": {"enabled": true}},
                    "withdraw": {"USD": {"enabled": false}},
                    "transaction": {"enabled": true, "authentication_required": true}
                })),
                "https://anchor.example/sep24/info" => Ok(serde_json::json!({
                    "deposit": {"USD": {"enabled": true}},
                    "withdraw": {"USD": {"enabled": true}}
                })),
                _ => Err(format!("unexpected URL: {url}")),
            })
            .unwrap();

        assert!(capabilities.sep6_deposit);
        assert!(!capabilities.sep6_withdraw);
        assert_eq!(capabilities.sep6_deposit_info["enabled"], true);
        assert_eq!(capabilities.sep6_withdraw_info["enabled"], false);
        assert_eq!(capabilities.sep6_transaction_info["enabled"], true);
        assert_eq!(
            capabilities.sep6_transaction_info["authentication_required"],
            true
        );
        assert!(capabilities.sep24_deposit);
        assert!(capabilities.sep24_withdraw);
        assert_eq!(
            capabilities.web_auth_for_contracts_url.as_deref(),
            Some("https://anchor.example/sep45/auth")
        );
        assert_eq!(capabilities.web_auth_contract_id.as_deref(), Some(CONTRACT));
        assert!(capabilities.sep10_auth);
        assert!(capabilities.sep45_auth);
        assert!(capabilities.warnings.is_empty());
    }

    #[test]
    fn discovery_rejects_same_code_from_different_issuer() {
        let document = r#"TRANSFER_SERVER = "https://anchor.example/sep6"
[[CURRENCIES]]
code = "USD"
issuer = "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBU4I"
"#;
        let capabilities = capabilities_from_document(&asset(), "anchor.example", document, |_| {
            panic!("info must not be loaded for a mismatched asset")
        })
        .unwrap();
        assert!(capabilities.sep6_url.is_none());
        assert_eq!(
            capabilities.warnings,
            vec!["stellar.toml does not list this exact asset"]
        );
    }

    #[test]
    fn discovery_requires_exact_case_and_issuer_identity() {
        for currency in [
            format!(
                r#"[[CURRENCIES]]
code = "usd"
issuer = "{ISSUER}"
"#
            ),
            r#"[[CURRENCIES]]
code = "USD"
"#
            .to_owned(),
        ] {
            let document = format!("TRANSFER_SERVER = \"https://anchor.example/sep6\"\n{currency}");
            let capabilities =
                capabilities_from_document(&asset(), "anchor.example", &document, |_| {
                    panic!("info must not be loaded for a mismatched asset")
                })
                .unwrap();
            assert!(capabilities.sep6_url.is_none());
            assert_eq!(
                capabilities.warnings,
                vec!["stellar.toml does not list this exact asset"]
            );
        }
    }

    #[test]
    fn discovery_info_keys_are_case_sensitive() {
        let document = format!(
            r#"TRANSFER_SERVER = "https://anchor.example/sep6"
[[CURRENCIES]]
code = "USD"
issuer = "{ISSUER}"
"#
        );
        let capabilities =
            capabilities_from_document(&asset(), "anchor.example", &document, |_| {
                Ok(serde_json::json!({
                    "deposit": {"usd": {"enabled": true}},
                    "withdraw": {"usd": {"enabled": true}}
                }))
            })
            .unwrap();
        assert!(!capabilities.sep6_deposit);
        assert!(!capabilities.sep6_withdraw);
    }

    #[test]
    fn discovery_keeps_info_failure_as_warning() {
        let document = format!(
            r#"TRANSFER_SERVER = "https://anchor.example/sep6"
[[CURRENCIES]]
code = "USD"
issuer = "{ISSUER}"
"#
        );
        let capabilities =
            capabilities_from_document(&asset(), "anchor.example", &document, |_| {
                Err("offline".to_owned())
            })
            .unwrap();
        assert_eq!(
            capabilities.sep6_url.as_deref(),
            Some("https://anchor.example/sep6")
        );
        assert_eq!(
            capabilities.warnings,
            vec!["SEP-6 /info unavailable: offline"]
        );
    }

    #[test]
    fn authentication_identifiers_are_account_kind_checked() {
        let valid: TomlValue = toml::from_str(&format!(
            r#"SIGNING_KEY = "{ISSUER}"
WEB_AUTH_CONTRACT_ID = "{CONTRACT}""#
        ))
        .unwrap();
        assert_eq!(
            account_identifier(&valid, "SIGNING_KEY", SdkAccountKind::Classic)
                .unwrap()
                .as_deref(),
            Some(ISSUER)
        );
        assert_eq!(
            account_identifier(&valid, "WEB_AUTH_CONTRACT_ID", SdkAccountKind::Contract)
                .unwrap()
                .as_deref(),
            Some(CONTRACT)
        );

        assert!(account_identifier(&valid, "SIGNING_KEY", SdkAccountKind::Contract).is_err());
        assert!(
            account_identifier(&valid, "WEB_AUTH_CONTRACT_ID", SdkAccountKind::Classic).is_err()
        );
    }

    #[test]
    fn sep24_accepts_complete_sep45_metadata_without_sep10() {
        let document = format!(
            r#"TRANSFER_SERVER_SEP0024 = "https://anchor.example/sep24"
WEB_AUTH_FOR_CONTRACTS_ENDPOINT = "https://anchor.example/sep45/auth"
SIGNING_KEY = "{ISSUER}"
WEB_AUTH_CONTRACT_ID = "{CONTRACT}"
[[CURRENCIES]]
code = "USD"
issuer = "{ISSUER}"
"#
        );
        let capabilities =
            capabilities_from_document(&asset(), "anchor.example", &document, |url| {
                assert_eq!(url, "https://anchor.example/sep24/info");
                Ok(serde_json::json!({
                    "deposit": {"USD": {"enabled": true}},
                    "withdraw": {"USD": {"enabled": true}}
                }))
            })
            .unwrap();

        assert!(capabilities.sep24_deposit);
        assert!(capabilities.sep24_withdraw);
        assert!(capabilities.web_auth_url.is_none());
        assert!(!capabilities.sep10_auth);
        assert!(capabilities.sep45_auth);
        assert!(capabilities.warnings.is_empty());
    }

    #[test]
    fn transfer_protocol_prefers_sep24_and_falls_back_to_sep6() {
        let capabilities = transfer_capabilities();
        assert_eq!(
            select_anchor_transfer_protocol(&capabilities, AnchorTransferKind::Deposit).unwrap(),
            AnchorProtocol::Sep24
        );

        let mut fallback = capabilities.clone();
        fallback.sep10_auth = false;
        fallback.web_auth_url = None;
        fallback.sep24_deposit = true;
        fallback.sep6_deposit_info = serde_json::json!({
            "enabled": true,
            "authentication_required": false,
            "funding_methods": ["WIRE"]
        });
        assert_eq!(
            select_anchor_transfer_protocol(&fallback, AnchorTransferKind::Deposit).unwrap(),
            AnchorProtocol::Sep6
        );
    }

    #[test]
    fn authenticated_sep6_requires_classic_sep10_metadata() {
        let mut capabilities = transfer_capabilities();
        capabilities.sep24_withdraw = false;
        capabilities.sep10_auth = false;
        capabilities.web_auth_url = None;

        assert_eq!(
            select_anchor_transfer_protocol(&capabilities, AnchorTransferKind::Withdraw).unwrap_err(),
            "SEP-6 withdraw requires authentication, but no complete Classic SEP-10 path is available"
        );
    }

    #[test]
    fn sep6_autofills_single_funding_method_and_requires_choice_for_many() {
        let capabilities = transfer_capabilities();
        let fields =
            sep6_request_fields(&capabilities, AnchorTransferKind::Deposit, &BTreeMap::new())
                .unwrap();
        assert_eq!(
            fields.get("funding_method").map(String::as_str),
            Some("WIRE")
        );

        let mut many = capabilities.clone();
        many.sep6_deposit_info = serde_json::json!({
            "enabled": true,
            "funding_methods": ["WIRE", "ACH"]
        });
        assert_eq!(
            sep6_request_fields(&many, AnchorTransferKind::Deposit, &BTreeMap::new()).unwrap_err(),
            "SEP-6 deposit requires an explicit funding_method because multiple methods are advertised"
        );
    }

    #[test]
    fn sep6_transport_requires_token_when_anchor_requires_authentication() {
        let capabilities = transfer_capabilities();

        assert_eq!(
            start_anchor_sep6_transfer(
                ISSUER,
                &asset(),
                &capabilities,
                AnchorTransferKind::Withdraw,
                &BTreeMap::new(),
                None,
            )
            .unwrap_err(),
            "SEP-6 withdraw requires SEP-10 authentication"
        );
        assert_eq!(
            start_anchor_sep6_transfer(
                ISSUER,
                &asset(),
                &capabilities,
                AnchorTransferKind::Withdraw,
                &BTreeMap::new(),
                Some(""),
            )
            .unwrap_err(),
            "SEP-6 withdraw requires SEP-10 authentication"
        );
    }

    #[test]
    fn sep6_request_binds_asset_account_and_user_fields() {
        let mut fields = BTreeMap::new();
        fields.insert("funding_method".to_owned(), "WIRE".to_owned());
        fields.insert("amount".to_owned(), "5".to_owned());
        let url = sep6_request_url(
            "https://anchor.example/sep6/",
            AnchorTransferKind::Withdraw,
            &asset(),
            ISSUER,
            &fields,
        )
        .unwrap();
        let pairs = url.query_pairs().collect::<BTreeMap<_, _>>();
        assert_eq!(url.path(), "/sep6/withdraw");
        assert_eq!(
            pairs.get("asset_code").map(|value| value.as_ref()),
            Some("USD")
        );
        assert_eq!(
            pairs.get("account").map(|value| value.as_ref()),
            Some(ISSUER)
        );
        assert_eq!(
            pairs.get("funding_method").map(|value| value.as_ref()),
            Some("WIRE")
        );
        assert_eq!(pairs.get("amount").map(|value| value.as_ref()), Some("5"));
    }

    #[test]
    fn sep24_transport_rejects_empty_token_before_http() {
        let capabilities = transfer_capabilities();
        assert_eq!(
            start_anchor_sep24_transfer(
                ISSUER,
                &asset(),
                &capabilities,
                AnchorTransferKind::Deposit,
                &BTreeMap::new(),
                "",
            )
            .unwrap_err(),
            "SEP-24 requires a non-empty SEP-10 token"
        );
    }

    #[test]
    fn sep24_interactive_response_requires_expected_type_and_safe_url() {
        let result = parse_sep24_interactive_response(&serde_json::json!({
            "type": "interactive_customer_info_needed",
            "url": "https://anchor.example/interactive/123",
            "id": "transfer-123"
        }))
        .unwrap();
        assert_eq!(result.url, "https://anchor.example/interactive/123");
        assert_eq!(result.transaction_id, "transfer-123");

        assert!(parse_sep24_interactive_response(&serde_json::json!({
            "type": "wrong",
            "url": "https://anchor.example/interactive/123"
        }))
        .is_err());
        assert!(parse_sep24_interactive_response(&serde_json::json!({
            "type": "interactive_customer_info_needed",
            "url": "https://anchor.example/interactive/123"
        }))
        .is_err());
        assert!(parse_sep24_interactive_response(&serde_json::json!({
            "type": "interactive_customer_info_needed",
            "url": "http://anchor.example/interactive/123",
            "id": "transfer-123"
        }))
        .is_err());
    }

    #[test]
    fn status_protocol_prefers_sep24_and_allows_explicit_sep6() {
        let capabilities = transfer_capabilities();
        assert!(select_anchor_status_protocol(&capabilities, None)
            .unwrap_err()
            .contains("both SEP-24 and SEP-6"));
        assert_eq!(
            select_anchor_status_protocol(&capabilities, Some(AnchorProtocol::Sep24)).unwrap(),
            AnchorProtocol::Sep24
        );
        assert_eq!(
            select_anchor_status_protocol(&capabilities, Some(AnchorProtocol::Sep6)).unwrap(),
            AnchorProtocol::Sep6
        );

        let mut fallback = capabilities.clone();
        fallback.sep24_url = None;
        assert_eq!(
            select_anchor_status_protocol(&fallback, None).unwrap(),
            AnchorProtocol::Sep6
        );
    }

    #[test]
    fn sep6_status_respects_transaction_auth_metadata() {
        let mut capabilities = transfer_capabilities();
        capabilities.sep24_url = None;
        capabilities.sep10_auth = false;
        capabilities.web_auth_url = None;

        assert_eq!(
            select_anchor_status_protocol(&capabilities, None).unwrap_err(),
            "SEP-6 transaction status requires a complete Classic SEP-10 authentication path"
        );

        capabilities.sep6_transaction_info = serde_json::json!({
            "enabled": true,
            "authentication_required": false
        });
        assert_eq!(
            select_anchor_status_protocol(&capabilities, None).unwrap(),
            AnchorProtocol::Sep6
        );
    }

    #[test]
    fn anchor_transaction_response_validates_id_and_status() {
        let transaction = parse_anchor_transaction_response(
            &serde_json::json!({
                "transaction": {
                    "id": "tx-1",
                    "kind": "withdrawal",
                    "status": "pending_anchor"
                }
            }),
            "tx-1",
        )
        .unwrap();
        assert_eq!(transaction["status"], "pending_anchor");

        assert!(parse_anchor_transaction_response(
            &serde_json::json!({
                "transaction": {"id": "other", "status": "pending_anchor"}
            }),
            "tx-1",
        )
        .is_err());
        assert!(parse_anchor_transaction_response(
            &serde_json::json!({"transaction": {"id": "tx-1"}}),
            "tx-1",
        )
        .is_err());
    }

    #[test]
    fn withdrawal_payment_requires_ready_status_and_parses_memo() {
        let payment = anchor_withdrawal_payment_from_transaction(
            &serde_json::json!({
                "id": "tx-1",
                "kind": "withdrawal",
                "status": "pending_user_transfer_start",
                "from": ISSUER,
                "withdraw_anchor_account": ISSUER,
                "withdraw_memo_type": "id",
                "withdraw_memo": "123",
                "amount_in": "5.2500000",
                "amount_in_asset": format!("stellar:USD:{ISSUER}")
            }),
            ISSUER,
            &asset(),
        )
        .unwrap();
        assert_eq!(payment.destination, ISSUER);
        assert_eq!(payment.amount, "5.2500000");
        assert_eq!(payment.memo, PaymentMemo::Id(123));

        assert!(anchor_withdrawal_payment_from_transaction(
            &serde_json::json!({
                "kind": "withdrawal",
                "status": "pending_anchor",
                "from": ISSUER,
                "withdraw_anchor_account": ISSUER,
                "amount_in": "5"
            }),
            ISSUER,
            &asset(),
        )
        .is_err());

        assert!(anchor_withdrawal_payment_from_transaction(
            &serde_json::json!({
                "kind": "withdrawal",
                "status": "pending_user_transfer_start",
                "from": SERVER_PUBLIC,
                "withdraw_anchor_account": ISSUER,
                "amount_in": "5"
            }),
            ISSUER,
            &asset(),
        )
        .is_err());

        assert!(anchor_withdrawal_payment_from_transaction(
            &serde_json::json!({
                "kind": "withdrawal",
                "status": "pending_user_transfer_start",
                "from": ISSUER,
                "withdraw_anchor_account": ISSUER,
                "amount_in": "5",
                "amount_in_asset": "stellar:EUR:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
            }),
            ISSUER,
            &asset(),
        )
        .is_err());
    }

    #[test]
    fn anchor_endpoints_require_https_without_credentials() {
        let insecure: TomlValue =
            toml::from_str(r#"TRANSFER_SERVER = "http://anchor.example/sep6""#).unwrap();
        assert!(endpoint(&insecure, "TRANSFER_SERVER").is_err());

        let credentialed: TomlValue =
            toml::from_str(r#"TRANSFER_SERVER = "https://user:pass@anchor.example/sep6""#).unwrap();
        assert!(endpoint(&credentialed, "TRANSFER_SERVER").is_err());
    }

    #[test]
    fn anchor_http_redirects_are_rejected() {
        assert!(reject_anchor_redirect(200, "https://anchor.example/info").is_ok());
        assert!(reject_anchor_redirect(403, "https://anchor.example/info").is_ok());
        assert_eq!(
            reject_anchor_redirect(302, "https://anchor.example/info").unwrap_err(),
            "Anchor endpoint https://anchor.example/info returned HTTP 302; redirects are not allowed"
        );
    }

    #[test]
    fn home_domain_rejects_urls_and_paths() {
        assert_eq!(valid_domain("Anchor.Example.").unwrap(), "anchor.example");
        assert!(valid_domain("https://anchor.example").is_err());
        assert!(valid_domain("anchor.example/path").is_err());
    }
}

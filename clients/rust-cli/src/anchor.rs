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
use url::Url;
use zeroize::Zeroizing;

use crate::storage::{WalletRecord, WalletStorage};
use crate::transaction_flow::{
    has_valid_transaction_signature, network_client, network_passphrase, parse_transaction_xdr,
    resolve_local_signing_wallet, sign_transaction_xdr_with_wallet,
};

const MAX_ANCHOR_DOCUMENT_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct IssuedAsset {
    code: String,
    issuer: String,
}

impl IssuedAsset {
    fn parse(value: &str) -> Result<Self, String> {
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

    fn display(&self) -> String {
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
    pub sep24_deposit: bool,
    pub sep24_withdraw: bool,
    pub warnings: Vec<String>,
}

pub fn command_anchor(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "discover" => command_discover(network, &arguments[1..]),
        "auth" => command_auth(storage, network, &arguments[1..]),
        _ => Err(usage().to_owned()),
    }
}

fn command_discover(network: &str, arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() {
        return Err(usage().to_owned());
    }
    let asset = IssuedAsset::parse(&arguments[0])?;
    let mut json = false;
    for argument in &arguments[1..] {
        match argument.as_str() {
            "--json" => json = true,
            _ => return Err(usage().to_owned()),
        }
    }

    let (home_domain, capabilities) = resolve_anchor(network, &asset)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "asset": asset.display(),
                "network": network,
                "capabilities": capabilities,
            }))
            .map_err(|error| format!("unable to encode anchor capabilities: {error}"))?
        );
        return Ok(());
    }

    println!("Anchor · {} [{}]", asset.display(), network);
    println!("Domain:     {}", capabilities.domain);
    println!(
        "SEP-6:      deposit={} withdraw={}{}",
        yes_no(capabilities.sep6_deposit),
        yes_no(capabilities.sep6_withdraw),
        capabilities
            .sep6_url
            .as_deref()
            .map(|url| format!(" · {url}"))
            .unwrap_or_default()
    );
    println!(
        "SEP-24:     deposit={} withdraw={}{}",
        yes_no(capabilities.sep24_deposit),
        yes_no(capabilities.sep24_withdraw),
        capabilities
            .sep24_url
            .as_deref()
            .map(|url| format!(" · {url}"))
            .unwrap_or_default()
    );
    if let Some(url) = &capabilities.web_auth_url {
        println!("SEP-10:     {url}");
    }
    if let Some(url) = &capabilities.web_auth_for_contracts_url {
        println!("SEP-45:     {url}");
    }
    if let Some(url) = &capabilities.kyc_url {
        println!("KYC:        {url}");
    }
    for warning in &capabilities.warnings {
        println!("Warning:    {warning}");
    }
    Ok(())
}

fn command_auth(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    if arguments.is_empty() {
        return Err(usage().to_owned());
    }
    let asset = IssuedAsset::parse(&arguments[0])?;
    let mut wallet = None;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--wallet requires a wallet name".to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }

    let (home_domain, capabilities) = resolve_anchor(network, &asset)?;
    let web_auth_endpoint = capabilities.web_auth_url.as_deref().ok_or_else(|| {
        format!("{} does not advertise a SEP-10 WEB_AUTH_ENDPOINT", capabilities.domain)
    })?;
    let signing_key = capabilities.signing_key.as_deref().ok_or_else(|| {
        format!("{} does not advertise a SEP-10 SIGNING_KEY", capabilities.domain)
    })?;
    let record = resolve_local_signing_wallet(storage, network, wallet)?;
    let token = authenticate_sep10(
        &record,
        network,
        &home_domain,
        web_auth_endpoint,
        signing_key,
    )?;

    println!("Authenticated · {} [{}]", capabilities.domain, network);
    println!("Wallet:        {}", record.name);
    println!("Address:       {}", record.address);
    println!("Token:         verified in memory, then discarded");
    drop(token);
    Ok(())
}

fn resolve_anchor(
    network: &str,
    asset: &IssuedAsset,
) -> Result<(String, AnchorCapabilities), String> {
    let issuer = network_client(network)?.get_account(&asset.issuer)?;
    let home_domain = issuer
        .get("home_domain")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Asset issuer {} has no home_domain", asset.issuer))?;
    let home_domain = valid_domain(home_domain)?;
    let capabilities = discover(asset, &home_domain)?;
    Ok((home_domain, capabilities))
}

fn discover(asset: &IssuedAsset, home_domain: &str) -> Result<AnchorCapabilities, String> {
    let domain = valid_domain(home_domain)?;
    let toml_url = format!("https://{domain}/.well-known/stellar.toml");
    let document = fetch_text(&toml_url, "stellar.toml")?;
    capabilities_from_document(asset, &domain, &document, fetch_json)
}

fn capabilities_from_document<F>(
    asset: &IssuedAsset,
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
    let sep6_requires_auth = [&sep6_deposit_info, &sep6_withdraw_info]
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

fn authenticate_sep10(
    record: &WalletRecord,
    network: &str,
    home_domain: &str,
    web_auth_endpoint: &str,
    signing_key: &str,
) -> Result<Zeroizing<String>, String> {
    let challenge = request_sep10_challenge(web_auth_endpoint, &record.address)?;
    validate_sep10_network_passphrase(network, challenge.network_passphrase.as_deref())?;

    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before Unix epoch".to_owned())?
        .as_secs();
    let verified_xdr = verify_sep10_challenge(
        &challenge.transaction,
        network,
        &record.address,
        signing_key,
        home_domain,
        web_auth_endpoint,
        now_unix,
    )?;
    let signed_xdr = sign_transaction_xdr_with_wallet(record, network, verified_xdr)?;
    let signed_envelope = parse_transaction_xdr(&signed_xdr)?;
    if !has_valid_transaction_signature(&signed_envelope, network, &record.address)? {
        return Err("Fresnica SDK did not return a valid client signature for SEP-10".to_owned());
    }

    exchange_sep10_challenge(web_auth_endpoint, &STANDARD.encode(signed_xdr))
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
    serde_json::from_value(value)
        .map_err(|error| format!("Invalid SEP-10 challenge response from {web_auth_endpoint}: {error}"))
}

fn sep10_challenge_url(
    web_auth_endpoint: &str,
    account: &str,
) -> Result<Url, String> {
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
        .send_json(serde_json::json!({"transaction": signed_transaction}))
        .map_err(|error| format!("Unable to exchange SEP-10 challenge at {web_auth_endpoint}: {error}"))?;
    let value = response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_DOCUMENT_BYTES)
        .read_json::<Sep10TokenResponse>()
        .map_err(|error| format!("Invalid SEP-10 token response from {web_auth_endpoint}: {error}"))?;
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
            return Err("SEP-10 challenge contains an unexpected client_domain operation".to_owned());
        }
        if operation.source_account.as_ref() != Some(&server_account) {
            return Err("SEP-10 additional operation source must be SIGNING_KEY".to_owned());
        }
        if key == "web_auth_domain" {
            if web_auth_domain_seen {
                return Err("SEP-10 challenge contains duplicate web_auth_domain operations".to_owned());
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
    let account = AccountId::from_str(address).map_err(|_| format!("{label} must be a Classic G address"))?;
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
    let url = Url::parse(endpoint).map_err(|_| "WEB_AUTH_ENDPOINT must be a valid URL".to_owned())?;
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

fn currency_matches(document: &TomlValue, asset: &IssuedAsset) -> bool {
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
            .is_some_and(|code| code.eq_ignore_ascii_case(&asset.code));
        if !code_matches {
            return false;
        }
        table
            .get("issuer")
            .and_then(TomlValue::as_str)
            .is_none_or(|issuer| issuer == asset.issuer)
    })
}

fn asset_info<'a>(section: Option<&'a JsonValue>, code: &str) -> Option<&'a JsonValue> {
    let section = section?.as_object()?;
    let value = section
        .get(code)
        .or_else(|| section.get(&code.to_ascii_uppercase()))
        .or_else(|| section.get(&code.to_ascii_lowercase()))?;
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
        .call()
        .map_err(|error| format!("Unable to load {label} from {url}: {error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_DOCUMENT_BYTES)
        .read_to_string()
        .map_err(|error| format!("Unable to read {label} from {url}: {error}"))
}

fn fetch_json(url: &str) -> Result<JsonValue, String> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| format!("Unable to load {url}: {error}"))?;
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

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn usage() -> &'static str {
    "usage:\n  fresnica [--network mainnet|testnet] anchor discover CODE:GISSUER [--json]\n  fresnica [--home PATH] [--network mainnet|testnet] anchor auth CODE:GISSUER [--wallet NAME]"
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

    fn asset() -> IssuedAsset {
        IssuedAsset::parse(&format!("USD:{ISSUER}")).unwrap()
    }

    #[test]
    fn sep10_challenge_request_binds_account_without_optional_extensions() {
        let url = sep10_challenge_url(WEB_AUTH_ENDPOINT, ISSUER).unwrap();
        let pairs = url.query_pairs().collect::<Vec<_>>();

        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("auth.example.com"));
        assert!(pairs.iter().any(|(key, value)| key == "account" && value == ISSUER));
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

        let wrong_auth_domain = challenge_envelope(
            ISSUER,
            HOME_DOMAIN,
            "evil.example",
            NOW - 30,
            NOW + 300,
            64,
        );
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
        assert!(IssuedAsset::parse("USD").is_err());
        assert!(IssuedAsset::parse("XLM").is_err());
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
        let capabilities = capabilities_from_document(&asset(), "anchor.example", &document, |url| {
            match url {
                "https://anchor.example/sep6/info" => Ok(serde_json::json!({
                    "deposit": {"USD": {"enabled": true}},
                    "withdraw": {"USD": {"enabled": false}}
                })),
                "https://anchor.example/sep24/info" => Ok(serde_json::json!({
                    "deposit": {"USD": {"enabled": true}},
                    "withdraw": {"USD": {"enabled": true}}
                })),
                _ => Err(format!("unexpected URL: {url}")),
            }
        })
        .unwrap();

        assert!(capabilities.sep6_deposit);
        assert!(!capabilities.sep6_withdraw);
        assert_eq!(capabilities.sep6_deposit_info["enabled"], true);
        assert_eq!(capabilities.sep6_withdraw_info["enabled"], false);
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
    fn discovery_keeps_info_failure_as_warning() {
        let document = r#"TRANSFER_SERVER = "https://anchor.example/sep6"
[[CURRENCIES]]
code = "USD"
"#;
        let capabilities = capabilities_from_document(&asset(), "anchor.example", document, |_| {
            Err("offline".to_owned())
        })
        .unwrap();
        assert_eq!(capabilities.sep6_url.as_deref(), Some("https://anchor.example/sep6"));
        assert_eq!(capabilities.warnings, vec!["SEP-6 /info unavailable: offline"]);
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
        assert!(account_identifier(
            &valid,
            "WEB_AUTH_CONTRACT_ID",
            SdkAccountKind::Classic
        )
        .is_err());
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
        let capabilities = capabilities_from_document(&asset(), "anchor.example", &document, |url| {
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
    fn anchor_endpoints_require_https_without_credentials() {
        let insecure: TomlValue = toml::from_str(
            r#"TRANSFER_SERVER = "http://anchor.example/sep6""#,
        )
        .unwrap();
        assert!(endpoint(&insecure, "TRANSFER_SERVER").is_err());

        let credentialed: TomlValue = toml::from_str(
            r#"TRANSFER_SERVER = "https://user:pass@anchor.example/sep6""#,
        )
        .unwrap();
        assert!(endpoint(&credentialed, "TRANSFER_SERVER").is_err());
    }

    #[test]
    fn home_domain_rejects_urls_and_paths() {
        assert_eq!(valid_domain("Anchor.Example.").unwrap(), "anchor.example");
        assert!(valid_domain("https://anchor.example").is_err());
        assert!(valid_domain("anchor.example/path").is_err());
    }
}

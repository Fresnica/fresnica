use fresnica_sdk::{FresnicaSdk, SdkAccountKind};
use serde::Serialize;
use serde_json::Value as JsonValue;
use toml::Value as TomlValue;
use url::Url;

use crate::transaction_flow::network_client;

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
    pub signing_key: Option<String>,
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

pub fn command_anchor(network: &str, arguments: &[String]) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "discover" => command_discover(network, &arguments[1..]),
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

    let issuer = network_client(network)?.get_account(&asset.issuer)?;
    let home_domain = issuer
        .get("home_domain")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Asset issuer {} has no home_domain", asset.issuer))?;
    let capabilities = discover(&asset, home_domain)?;

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
    if let Some(url) = &capabilities.kyc_url {
        println!("KYC:        {url}");
    }
    for warning in &capabilities.warnings {
        println!("Warning:    {warning}");
    }
    Ok(())
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
            signing_key: None,
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
    let signing_key = text(&document, "SIGNING_KEY");
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

    if (sep24_deposit || sep24_withdraw) && (web_auth_url.is_none() || signing_key.is_none()) {
        warnings.push(
            "SEP-24 is advertised but this Classic-account client has incomplete SEP-10 metadata"
                .to_owned(),
        );
    }

    Ok(AnchorCapabilities {
        domain: domain.to_owned(),
        sep6_url,
        sep24_url,
        web_auth_url,
        signing_key,
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
    "usage: fresnica [--network mainnet|testnet] anchor discover CODE:GISSUER [--json]"
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    fn asset() -> IssuedAsset {
        IssuedAsset::parse(&format!("USD:{ISSUER}")).unwrap()
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
SIGNING_KEY = "{ISSUER}"

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

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::asset::AssetId;
use crate::service::FresnicaClient;

const CACHE_VERSION: u8 = 1;
const STELLAR_EXPERT_ASSETS: &str = "https://api.stellar.expert/explorer/public/asset";
pub const MAX_ASSET_CATALOG_LIMIT: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetCatalogEntry {
    pub identity: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub organization: Option<String>,
    pub source: String,
}

impl AssetCatalogEntry {
    pub fn is_native(&self) -> bool {
        self.identity == "XLM"
    }

    fn from_identity(
        identity: &str,
        domain: Option<String>,
        name: Option<String>,
        organization: Option<String>,
        source: &str,
    ) -> Result<Self, String> {
        let asset = AssetId::parse(identity)?;
        Ok(Self {
            identity: asset.display(),
            domain: clean_optional(domain),
            name: clean_optional(name),
            organization: clean_optional(organization),
            source: source.trim().to_owned(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetCatalogSnapshot {
    pub entries: Vec<AssetCatalogEntry>,
    pub refreshed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AssetCatalogCache {
    version: u8,
    network: String,
    entries: Vec<AssetCatalogEntry>,
}

struct AssetCatalog {
    network: String,
    path: PathBuf,
}

impl FresnicaClient {
    pub fn asset_catalog(
        &self,
        limit: usize,
        refresh: bool,
    ) -> Result<AssetCatalogSnapshot, String> {
        AssetCatalog::new(self.storage().home(), self.network()).load(limit, refresh)
    }
}

impl AssetCatalog {
    fn new(home: &Path, network: &str) -> Self {
        Self {
            network: network.to_owned(),
            path: home.join(format!("asset-catalog-{network}.json")),
        }
    }

    fn load(&self, limit: usize, refresh: bool) -> Result<AssetCatalogSnapshot, String> {
        self.load_with(limit, refresh, fetch_stellar_expert)
    }

    fn load_with<F>(
        &self,
        limit: usize,
        refresh: bool,
        fetch: F,
    ) -> Result<AssetCatalogSnapshot, String>
    where
        F: FnOnce(usize) -> Result<Vec<AssetCatalogEntry>, String>,
    {
        validate_limit(limit)?;
        let cached = self.read_cache(limit)?;
        if self.network != "mainnet" || !refresh {
            return Ok(AssetCatalogSnapshot {
                entries: cached,
                refreshed: false,
            });
        }

        let fresh = fetch(limit)
            .map(|entries| normalize_provider_entries(entries, limit))
            .unwrap_or_default();
        if fresh.len() <= 1 {
            return Ok(AssetCatalogSnapshot {
                entries: cached,
                refreshed: false,
            });
        }

        self.write_cache(&fresh[1..])?;
        Ok(AssetCatalogSnapshot {
            entries: fresh,
            refreshed: true,
        })
    }

    fn read_cache(&self, limit: usize) -> Result<Vec<AssetCatalogEntry>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(vec![native_entry()]);
            }
            Err(error) => {
                return Err(format!(
                    "unable to read asset catalog cache {}: {error}",
                    self.path.display()
                ));
            }
        };
        let cache: AssetCatalogCache = serde_json::from_str(&text)
            .map_err(|error| format!("invalid asset catalog cache: {error}"))?;
        if cache.version != CACHE_VERSION || cache.network != self.network {
            return Err("unsupported asset catalog cache".to_owned());
        }

        let mut entries = vec![native_entry()];
        let mut seen = HashSet::from(["XLM".to_owned()]);
        for raw in cache.entries {
            let entry = AssetCatalogEntry::from_identity(
                &raw.identity,
                raw.domain,
                raw.name,
                raw.organization,
                &raw.source,
            )
            .map_err(|_| "asset catalog cache contains an invalid asset identity".to_owned())?;
            if entry.is_native() || !seen.insert(entry.identity.clone()) {
                continue;
            }
            entries.push(entry);
            if entries.len() > limit {
                break;
            }
        }
        Ok(entries)
    }

    fn write_cache(&self, entries: &[AssetCatalogEntry]) -> Result<(), String> {
        let cache = AssetCatalogCache {
            version: CACHE_VERSION,
            network: self.network.clone(),
            entries: entries.to_vec(),
        };
        let text = serde_json::to_string_pretty(&cache)
            .map_err(|error| format!("unable to encode asset catalog cache: {error}"))?
            + "\n";
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("unable to create asset catalog directory: {error}"))?;
        }
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, text)
            .map_err(|error| format!("unable to write asset catalog cache: {error}"))?;
        #[cfg(windows)]
        if self.path.exists() {
            fs::remove_file(&self.path)
                .map_err(|error| format!("unable to replace asset catalog cache: {error}"))?;
        }
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("unable to replace asset catalog cache: {error}"));
        }
        Ok(())
    }
}

fn validate_limit(limit: usize) -> Result<(), String> {
    if !(1..=MAX_ASSET_CATALOG_LIMIT).contains(&limit) {
        return Err(format!(
            "asset catalog limit must be from 1 to {MAX_ASSET_CATALOG_LIMIT}"
        ));
    }
    Ok(())
}

fn native_entry() -> AssetCatalogEntry {
    AssetCatalogEntry {
        identity: "XLM".to_owned(),
        domain: None,
        name: None,
        organization: None,
        source: "native".to_owned(),
    }
}

fn normalize_provider_entries(
    entries: Vec<AssetCatalogEntry>,
    limit: usize,
) -> Vec<AssetCatalogEntry> {
    let mut normalized = vec![native_entry()];
    let mut seen = HashSet::from(["XLM".to_owned()]);
    for raw in entries {
        let Ok(entry) = AssetCatalogEntry::from_identity(
            &raw.identity,
            raw.domain,
            raw.name,
            raw.organization,
            &raw.source,
        ) else {
            continue;
        };
        if entry.is_native() || !seen.insert(entry.identity.clone()) {
            continue;
        }
        normalized.push(entry);
        if normalized.len() > limit {
            break;
        }
    }
    normalized
}

fn fetch_stellar_expert(limit: usize) -> Result<Vec<AssetCatalogEntry>, String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .https_only(true)
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let limit_text = limit.to_string();
    let mut response = agent
        .get(STELLAR_EXPERT_ASSETS)
        .query("sort", "rating")
        .query("order", "desc")
        .query("limit", limit_text)
        .call()
        .map_err(|error| format!("asset catalog refresh failed: {error}"))?;
    let payload: Value = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_json()
        .map_err(|error| format!("asset catalog response is invalid: {error}"))?;
    parse_stellar_expert(&payload)
}

fn parse_stellar_expert(payload: &Value) -> Result<Vec<AssetCatalogEntry>, String> {
    let records = payload
        .get("_embedded")
        .and_then(|value| value.get("records"))
        .and_then(Value::as_array)
        .ok_or_else(|| "asset catalog response is malformed".to_owned())?;
    let mut entries = Vec::new();
    for raw in records {
        let Some(encoded_asset) = raw.get("asset").and_then(Value::as_str) else {
            continue;
        };
        let Some((identity_with_issuer, _counter)) = encoded_asset.rsplit_once('-') else {
            continue;
        };
        let Some((code, issuer)) = identity_with_issuer.rsplit_once('-') else {
            continue;
        };
        let identity = format!("{code}:{issuer}");
        let toml = raw.get("tomlInfo").and_then(Value::as_object);
        let Ok(entry) = AssetCatalogEntry::from_identity(
            &identity,
            optional_text(raw.get("domain")),
            toml.and_then(|value| optional_text(value.get("name"))),
            toml.and_then(|value| optional_text(value.get("orgName"))),
            "stellar-expert",
        ) else {
            continue;
        };
        entries.push(entry);
    }
    Ok(entries)
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    fn temp_home(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fresnica-asset-catalog-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn issued(code: &str, source: &str) -> AssetCatalogEntry {
        AssetCatalogEntry::from_identity(
            &format!("{code}:{ISSUER}"),
            Some("example.org".to_owned()),
            Some(format!("{code} token")),
            None,
            source,
        )
        .unwrap()
    }

    #[test]
    fn testnet_never_fetches_mainnet_recommendations() {
        let catalog = AssetCatalog::new(&temp_home("testnet"), "testnet");
        let snapshot = catalog
            .load_with(10, true, |_| panic!("provider must not be called"))
            .unwrap();
        assert_eq!(snapshot.entries, vec![native_entry()]);
        assert!(!snapshot.refreshed);
    }

    #[test]
    fn refresh_failure_uses_valid_cache() {
        let home = temp_home("fallback");
        let catalog = AssetCatalog::new(&home, "mainnet");
        catalog.write_cache(&[issued("USD", "cache")]).unwrap();

        let snapshot = catalog
            .load_with(10, true, |_| Err("offline".to_owned()))
            .unwrap();
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[1].identity, format!("USD:{ISSUER}"));
        assert!(!snapshot.refreshed);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn fresh_results_are_deduplicated_and_cached() {
        let home = temp_home("fresh");
        let catalog = AssetCatalog::new(&home, "mainnet");
        let duplicate = issued("USD", "second");
        let snapshot = catalog
            .load_with(10, true, |_| {
                Ok(vec![
                    issued("USD", "first"),
                    duplicate,
                    issued("EUR", "first"),
                ])
            })
            .unwrap();
        assert!(snapshot.refreshed);
        assert_eq!(snapshot.entries.len(), 3);

        let cached = catalog.load_with(10, false, |_| unreachable!()).unwrap();
        assert_eq!(cached.entries, snapshot.entries);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn corrupt_cache_fails_explicitly() {
        let home = temp_home("corrupt");
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join("asset-catalog-mainnet.json"), "{}\n").unwrap();
        let catalog = AssetCatalog::new(&home, "mainnet");
        let error = catalog
            .load_with(10, false, |_| unreachable!())
            .unwrap_err();
        assert!(error.starts_with("invalid asset catalog cache:"));
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn parses_stellar_expert_exact_identity_and_metadata() {
        let payload = serde_json::json!({
            "_embedded": {
                "records": [
                    {
                        "asset": format!("USD-{ISSUER}-123"),
                        "domain": "example.org",
                        "tomlInfo": {
                            "name": "Example Dollar",
                            "orgName": "Example Org"
                        }
                    },
                    { "asset": "malformed" }
                ]
            }
        });
        let entries = parse_stellar_expert(&payload).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].identity, format!("USD:{ISSUER}"));
        assert_eq!(entries[0].domain.as_deref(), Some("example.org"));
        assert_eq!(entries[0].name.as_deref(), Some("Example Dollar"));
        assert_eq!(entries[0].organization.as_deref(), Some("Example Org"));
        assert_eq!(entries[0].source, "stellar-expert");
    }

    #[test]
    fn limit_is_bounded() {
        let catalog = AssetCatalog::new(&temp_home("limit"), "mainnet");
        assert_eq!(
            catalog.load_with(0, false, |_| unreachable!()).unwrap_err(),
            "asset catalog limit must be from 1 to 50"
        );
    }
}

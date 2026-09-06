use std::path::Path;

use serde_json::Value;

use crate::account_state::AccountState;
use crate::asset_catalog::AssetCatalog;
use crate::balance_state::AssetBalance;
use crate::contacts::ContactStore;
use crate::history_state::HistoryOperation;
use crate::horizon_gateway::{HorizonGateway, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL};
use crate::storage::{WalletRecord, WalletStorage};
use crate::transaction::PendingTransactionStore;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkProfile {
    network: String,
    horizon_url: String,
}

impl NetworkProfile {
    pub fn for_network(network: &str) -> Result<Self, String> {
        Ok(Self {
            network: network.to_owned(),
            horizon_url: horizon_url(network)?.to_owned(),
        })
    }

    pub fn network(&self) -> &str {
        &self.network
    }

    pub fn horizon_url(&self) -> &str {
        &self.horizon_url
    }

    pub fn with_horizon_url(mut self, horizon_url: &str) -> Result<Self, String> {
        self.horizon_url = validate_endpoint_url("Horizon", horizon_url)?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccountSnapshot {
    pub wallet: WalletRecord,
    pub account: AccountState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BalanceSnapshot {
    pub wallet: WalletRecord,
    pub balances: Vec<AssetBalance>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistorySnapshot {
    pub wallet: WalletRecord,
    pub operations: Vec<HistoryOperation>,
}

pub struct FresnicaClient {
    profile: NetworkProfile,
    storage: WalletStorage,
    contacts: ContactStore,
    pending_transactions: PendingTransactionStore,
    asset_catalog: AssetCatalog,
    gateway: HorizonGateway,
}

impl FresnicaClient {
    pub fn new(home: &Path, network: &str) -> Result<Self, String> {
        Self::from_profile(home, NetworkProfile::for_network(network)?)
    }

    pub fn from_profile(home: &Path, profile: NetworkProfile) -> Result<Self, String> {
        let gateway = HorizonGateway::new(profile.horizon_url());
        let storage = WalletStorage::new(home)?;
        let contacts = ContactStore::for_home(home);
        let pending_transactions = PendingTransactionStore::for_home(home);
        let asset_catalog = AssetCatalog::new(home, profile.network());
        Ok(Self {
            profile,
            storage,
            contacts,
            pending_transactions,
            asset_catalog,
            gateway,
        })
    }

    pub fn network(&self) -> &str {
        self.profile.network()
    }

    pub fn network_profile(&self) -> &NetworkProfile {
        &self.profile
    }

    pub fn storage(&self) -> &WalletStorage {
        &self.storage
    }

    pub(crate) fn gateway(&self) -> &HorizonGateway {
        &self.gateway
    }

    pub(crate) fn contact_store(&self) -> &ContactStore {
        &self.contacts
    }

    pub(crate) fn pending_transaction_store(&self) -> &PendingTransactionStore {
        &self.pending_transactions
    }

    pub(crate) fn asset_catalog_store(&self) -> &AssetCatalog {
        &self.asset_catalog
    }

    pub fn wallets(&self) -> Result<Vec<WalletRecord>, String> {
        Ok(self
            .storage
            .list()?
            .into_iter()
            .filter(|record| record.network.as_str() == self.network())
            .collect())
    }

    pub fn resolve_wallet(&self, name: Option<&str>) -> Result<WalletRecord, String> {
        let record = self.storage.resolve(name)?;
        if record.network.as_str() != self.network() {
            return Err(format!(
                "wallet \"{}\" is configured for {}; invoke with --network {}",
                record.name, record.network, record.network
            ));
        }
        Ok(record)
    }

    pub fn ledger_account(&self, address: &str) -> Result<Option<Value>, String> {
        self.gateway.get_account_optional(address)
    }

    pub fn account(&self, name: Option<&str>) -> Result<AccountSnapshot, String> {
        let wallet = self.resolve_wallet(name)?;
        let raw_account = self.gateway.get_account(&wallet.address)?;
        let account = AccountState::from_horizon(&raw_account)?;
        if account.account_id != wallet.address {
            return Err(format!(
                "Horizon returned account {} while loading {}",
                account.account_id, wallet.address
            ));
        }
        Ok(AccountSnapshot { wallet, account })
    }

    pub fn balances(&self, name: Option<&str>) -> Result<BalanceSnapshot, String> {
        let wallet = self.resolve_wallet(name)?;
        let account = self.gateway.get_account(&wallet.address)?;
        let raw_balances = account
            .get("balances")
            .and_then(Value::as_array)
            .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;
        let balances = raw_balances
            .iter()
            .map(AssetBalance::from_horizon)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BalanceSnapshot { wallet, balances })
    }

    pub fn history(&self, name: Option<&str>, limit: usize) -> Result<HistorySnapshot, String> {
        if !(1..=200).contains(&limit) {
            return Err("history limit must be from 1 to 200".to_owned());
        }
        let wallet = self.resolve_wallet(name)?;
        let raw_operations = self.gateway.get_operations(&wallet.address, limit)?;
        let operations = raw_operations
            .iter()
            .map(HistoryOperation::from_horizon)
            .collect();
        Ok(HistorySnapshot { wallet, operations })
    }
}

pub fn horizon_url(network: &str) -> Result<&'static str, String> {
    match network {
        "mainnet" => Ok(MAINNET_HORIZON_URL),
        "testnet" => Ok(TESTNET_HORIZON_URL),
        other => Err(format!("unknown network: {other}")),
    }
}

fn validate_endpoint_url(label: &str, value: &str) -> Result<String, String> {
    let value = value.trim().trim_end_matches('/');
    if value.is_empty() {
        return Err(format!("{label} URL must not be empty"));
    }
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return Err(format!("{label} URL must start with http:// or https://"));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::Map;

    use super::*;

    const ADDRESS: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    fn temp_home(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fresnica-client-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn validates_network_before_any_horizon_request() {
        let error = FresnicaClient::new(&temp_home("network"), "future-net")
            .err()
            .expect("invalid network should fail");
        assert_eq!(error, "unknown network: future-net");
    }

    #[test]
    fn reusable_client_filters_wallets_by_network() {
        let home = temp_home("wallets");
        let storage = WalletStorage::new(&home).unwrap();
        for (name, network) in [("main", "mainnet"), ("test", "testnet")] {
            storage
                .save(
                    &WalletRecord {
                        name: name.to_owned(),
                        address: ADDRESS.to_owned(),
                        wallet_type: "watch-only".to_owned(),
                        network: network.to_owned(),
                        secret: None,
                        metadata: Map::new(),
                    },
                    false,
                )
                .unwrap();
        }

        let client = FresnicaClient::new(&home, "testnet").unwrap();
        let wallets = client.wallets().unwrap();
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].name, "test");
    }

    #[test]
    fn network_profile_separates_network_identity_from_provider_endpoints() {
        let profile = NetworkProfile::for_network("testnet")
            .unwrap()
            .with_horizon_url("https://stellar.example/horizon/")
            .unwrap();

        assert_eq!(profile.network(), "testnet");
        assert_eq!(profile.horizon_url(), "https://stellar.example/horizon");

        let client = FresnicaClient::from_profile(&temp_home("profile"), profile.clone()).unwrap();
        assert_eq!(client.network_profile(), &profile);
    }

    #[test]
    fn network_profile_rejects_non_http_provider_endpoints() {
        let error = NetworkProfile::for_network("mainnet")
            .unwrap()
            .with_horizon_url("horizon.internal")
            .unwrap_err();
        assert_eq!(error, "Horizon URL must start with http:// or https://");
    }
}

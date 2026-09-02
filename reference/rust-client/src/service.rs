use std::path::Path;

use serde_json::Value;

use crate::horizon_gateway::{HorizonGateway, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL};
use crate::storage::{WalletRecord, WalletStorage};

#[derive(Clone, Debug, PartialEq)]
pub struct AccountSnapshot {
    pub wallet: WalletRecord,
    pub account: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BalanceSnapshot {
    pub wallet: WalletRecord,
    pub balances: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistorySnapshot {
    pub wallet: WalletRecord,
    pub operations: Vec<Value>,
}

pub struct FresnicaClient {
    network: String,
    storage: WalletStorage,
    gateway: HorizonGateway,
}

impl FresnicaClient {
    pub fn new(home: &Path, network: &str) -> Result<Self, String> {
        let gateway = HorizonGateway::new(horizon_url(network)?);
        Ok(Self {
            network: network.to_owned(),
            storage: WalletStorage::new(home)?,
            gateway,
        })
    }

    pub fn network(&self) -> &str {
        &self.network
    }

    pub fn storage(&self) -> &WalletStorage {
        &self.storage
    }

    pub(crate) fn gateway(&self) -> &HorizonGateway {
        &self.gateway
    }

    pub fn wallets(&self) -> Result<Vec<WalletRecord>, String> {
        Ok(self
            .storage
            .list()?
            .into_iter()
            .filter(|record| record.network == self.network)
            .collect())
    }

    pub fn resolve_wallet(&self, name: Option<&str>) -> Result<WalletRecord, String> {
        let record = self.storage.resolve(name)?;
        if record.network != self.network {
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
        let account = self.gateway.get_account(&wallet.address)?;
        Ok(AccountSnapshot { wallet, account })
    }

    pub fn balances(&self, name: Option<&str>) -> Result<BalanceSnapshot, String> {
        let AccountSnapshot { wallet, account } = self.account(name)?;
        let balances = account
            .get("balances")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;
        Ok(BalanceSnapshot { wallet, balances })
    }

    pub fn history(&self, name: Option<&str>, limit: usize) -> Result<HistorySnapshot, String> {
        if !(1..=200).contains(&limit) {
            return Err("history limit must be from 1 to 200".to_owned());
        }
        let wallet = self.resolve_wallet(name)?;
        let operations = self.gateway.get_operations(&wallet.address, limit)?;
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
}

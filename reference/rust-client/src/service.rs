use std::path::Path;

use serde_json::Value;

use crate::account_state::AccountState;
use crate::asset_catalog::AssetCatalog;
use crate::balance_state::AssetBalance;
use crate::contacts::ContactStore;
use crate::contract::{
    authorize_contract_invoke, contract_interface, prepare_contract_invoke,
    prepare_contract_invoke_outcome, sign_contract_invoke, submit_contract_invoke,
    ContractInterface, ContractInvokePreparation, ContractInvokeRequest, PreparedContractInvoke,
};
use crate::history_state::HistoryOperation;
use crate::horizon_gateway::{HorizonGateway, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL};
use crate::rpc_gateway::{RpcGateway, TESTNET_RPC_URL};
use crate::storage::{WalletRecord, WalletStorage};
use crate::transaction::{PendingTransactionStore, TransactionSubmission};
use crate::transaction_history::{HistoryTransaction, MAX_TRANSACTION_HISTORY_LIMIT};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkProfile {
    network: String,
    horizon_url: String,
    rpc_url: Option<String>,
}

impl NetworkProfile {
    pub fn for_network(network: &str) -> Result<Self, String> {
        let horizon_url = horizon_url(network)?.to_owned();
        let rpc_url = match network {
            "testnet" => Some(TESTNET_RPC_URL.to_owned()),
            "mainnet" => None,
            _ => unreachable!("horizon_url validated the network"),
        };
        Ok(Self {
            network: network.to_owned(),
            horizon_url,
            rpc_url,
        })
    }

    pub fn network(&self) -> &str {
        &self.network
    }

    pub fn horizon_url(&self) -> &str {
        &self.horizon_url
    }

    pub fn rpc_url(&self) -> Option<&str> {
        self.rpc_url.as_deref()
    }

    pub fn with_horizon_url(mut self, horizon_url: &str) -> Result<Self, String> {
        self.horizon_url = validate_endpoint_url("Horizon", horizon_url)?;
        Ok(self)
    }

    pub fn with_rpc_url(mut self, rpc_url: &str) -> Result<Self, String> {
        self.rpc_url = Some(validate_endpoint_url("Stellar RPC", rpc_url)?);
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

#[derive(Clone, Debug, PartialEq)]
pub struct TransactionHistorySnapshot {
    pub wallet: WalletRecord,
    pub transactions: Vec<HistoryTransaction>,
    pub next_cursor: Option<String>,
}

pub struct FresnicaClient {
    profile: NetworkProfile,
    storage: WalletStorage,
    contacts: ContactStore,
    pending_transactions: PendingTransactionStore,
    asset_catalog: AssetCatalog,
    gateway: HorizonGateway,
    rpc: Option<RpcGateway>,
}

impl FresnicaClient {
    pub fn new(home: &Path, network: &str) -> Result<Self, String> {
        Self::from_profile(home, NetworkProfile::for_network(network)?)
    }

    pub fn from_profile(home: &Path, profile: NetworkProfile) -> Result<Self, String> {
        let gateway = HorizonGateway::new(profile.horizon_url());
        let rpc = profile
            .rpc_url()
            .map(|rpc_url| RpcGateway::new(profile.network(), rpc_url))
            .transpose()?;
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
            rpc,
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

    fn rpc_gateway(&self) -> Result<&RpcGateway, String> {
        self.rpc.as_ref().ok_or_else(|| {
            format!(
                "No Stellar RPC endpoint configured for {}; configure one before using contract invoke",
                self.network()
            )
        })
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

    pub fn transaction_history(
        &self,
        name: Option<&str>,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<TransactionHistorySnapshot, String> {
        if !(1..=MAX_TRANSACTION_HISTORY_LIMIT).contains(&limit) {
            return Err(format!(
                "transaction history limit must be from 1 to {MAX_TRANSACTION_HISTORY_LIMIT}"
            ));
        }
        let wallet = self.resolve_wallet(name)?;
        let raw_transactions =
            self.gateway
                .get_account_transactions(&wallet.address, limit, cursor)?;
        let mut transactions = Vec::with_capacity(raw_transactions.len());
        for transaction in &raw_transactions {
            let transaction_hash = transaction
                .get("hash")
                .and_then(Value::as_str)
                .ok_or_else(|| "Horizon returned a transaction without hash".to_owned())?;
            let raw_operations = self.gateway.get_transaction_operations(transaction_hash)?;
            transactions.push(HistoryTransaction::from_horizon(
                transaction,
                &raw_operations,
            )?);
        }
        let next_cursor = (raw_transactions.len() == limit)
            .then(|| {
                transactions
                    .last()
                    .map(|transaction| transaction.paging_token.clone())
            })
            .flatten();
        Ok(TransactionHistorySnapshot {
            wallet,
            transactions,
            next_cursor,
        })
    }

    pub async fn contract_interface(&self, contract_id: &str) -> Result<ContractInterface, String> {
        contract_interface(self.rpc_gateway()?, contract_id).await
    }

    pub async fn prepare_contract_invoke(
        &self,
        request: ContractInvokeRequest,
    ) -> Result<PreparedContractInvoke, String> {
        prepare_contract_invoke(&self.storage, self.rpc_gateway()?, request).await
    }

    pub async fn prepare_contract_invoke_outcome(
        &self,
        request: ContractInvokeRequest,
    ) -> Result<ContractInvokePreparation, String> {
        prepare_contract_invoke_outcome(&self.storage, self.rpc_gateway()?, request).await
    }

    pub fn authorize_contract_invoke(
        &self,
        prepared: &mut PreparedContractInvoke,
        passcode: &str,
    ) -> Result<(), String> {
        authorize_contract_invoke(&self.storage, prepared, passcode)
    }

    pub fn sign_contract_invoke(
        &self,
        prepared: &mut PreparedContractInvoke,
        passcode: &str,
    ) -> Result<(), String> {
        sign_contract_invoke(&self.storage, prepared, &self.gateway, passcode)
    }

    pub async fn submit_contract_invoke(
        &self,
        prepared: &PreparedContractInvoke,
    ) -> Result<TransactionSubmission, String> {
        submit_contract_invoke(&self.storage, self.rpc_gateway()?, prepared).await
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
    fn validates_network_before_any_provider_request() {
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
            .unwrap()
            .with_rpc_url("https://stellar.example/rpc/")
            .unwrap();

        assert_eq!(profile.network(), "testnet");
        assert_eq!(profile.horizon_url(), "https://stellar.example/horizon");
        assert_eq!(profile.rpc_url(), Some("https://stellar.example/rpc"));

        let client = FresnicaClient::from_profile(&temp_home("profile"), profile.clone()).unwrap();
        assert_eq!(client.network_profile(), &profile);
    }

    #[test]
    fn network_profile_supplies_only_a_known_testnet_rpc_default() {
        let testnet = NetworkProfile::for_network("testnet").unwrap();
        let mainnet = NetworkProfile::for_network("mainnet").unwrap();

        assert_eq!(testnet.rpc_url(), Some(TESTNET_RPC_URL));
        assert_eq!(mainnet.rpc_url(), None);
    }

    #[test]
    fn network_profile_rejects_non_http_provider_endpoints() {
        let horizon_error = NetworkProfile::for_network("mainnet")
            .unwrap()
            .with_horizon_url("horizon.internal")
            .unwrap_err();
        assert_eq!(
            horizon_error,
            "Horizon URL must start with http:// or https://"
        );

        let rpc_error = NetworkProfile::for_network("mainnet")
            .unwrap()
            .with_rpc_url("rpc.internal")
            .unwrap_err();
        assert_eq!(
            rpc_error,
            "Stellar RPC URL must start with http:// or https://"
        );
    }

    #[test]
    fn contract_invoke_requires_rpc_when_profile_has_no_default() {
        let client = FresnicaClient::new(&temp_home("mainnet-no-rpc"), "mainnet").unwrap();
        let error = client.rpc_gateway().unwrap_err();
        assert_eq!(
            error,
            "No Stellar RPC endpoint configured for mainnet; configure one before using contract invoke"
        );
    }
}

use std::path::Path;

use serde_json::Value;

use crate::account_state::AccountState;
use crate::activity_state::{
    horizon_operation_cursor, ActivityTransaction, MAX_ACTIVITY_TRANSACTIONS,
};
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
pub struct ActivitySnapshot {
    pub wallet: WalletRecord,
    pub transactions: Vec<ActivityTransaction>,
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

    pub fn activity(&self, name: Option<&str>, limit: usize) -> Result<ActivitySnapshot, String> {
        if !(1..=MAX_ACTIVITY_TRANSACTIONS).contains(&limit) {
            return Err(format!(
                "activity limit must be from 1 to {MAX_ACTIVITY_TRANSACTIONS}"
            ));
        }
        let wallet = self.resolve_wallet(name)?;
        let transactions = collect_activity_transactions(limit, 200, |cursor| {
            self.gateway
                .get_operations_with_transactions(&wallet.address, 200, cursor)
        })?;
        Ok(ActivitySnapshot {
            wallet,
            transactions,
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

const MAX_ACTIVITY_PAGES: usize = 128;

fn collect_activity_transactions<F>(
    limit: usize,
    page_limit: usize,
    mut fetch_page: F,
) -> Result<Vec<ActivityTransaction>, String>
where
    F: FnMut(Option<&str>) -> Result<Vec<Value>, String>,
{
    if limit == 0 {
        return Err("activity transaction limit must be positive".to_owned());
    }
    if page_limit == 0 {
        return Err("activity page limit must be positive".to_owned());
    }
    let mut transactions = Vec::with_capacity(limit);
    let mut current: Option<ActivityTransaction> = None;
    let mut cursor: Option<String> = None;
    let mut pages_loaded = 0usize;

    loop {
        pages_loaded += 1;
        if pages_loaded > MAX_ACTIVITY_PAGES {
            return Err("Horizon activity pagination exceeded the safety bound".to_owned());
        }
        let page = fetch_page(cursor.as_deref())?;
        if page.is_empty() {
            if let Some(transaction) = current.take() {
                transactions.push(transaction.finish_descending_group());
            }
            transactions.truncate(limit);
            return Ok(transactions);
        }

        for raw in &page {
            let next = ActivityTransaction::from_horizon_joined_operation(raw)?;
            match current.as_mut() {
                Some(group) if group.transaction_hash == next.transaction_hash => {
                    group.merge_same_transaction(next)?;
                }
                Some(_) => {
                    let completed = current
                        .take()
                        .ok_or_else(|| "activity grouping lost its current transaction".to_owned())?
                        .finish_descending_group();
                    transactions.push(completed);
                    if transactions.len() == limit {
                        return Ok(transactions);
                    }
                    current = Some(next);
                }
                None => current = Some(next),
            }
        }

        if page.len() < page_limit {
            if let Some(transaction) = current.take() {
                transactions.push(transaction.finish_descending_group());
            }
            transactions.truncate(limit);
            return Ok(transactions);
        }

        let final_operation = page
            .last()
            .ok_or_else(|| "activity page unexpectedly became empty".to_owned())?;
        let next_cursor = horizon_operation_cursor(final_operation)?;
        if cursor.as_deref() == Some(next_cursor.as_str()) {
            return Err("Horizon activity cursor did not advance".to_owned());
        }
        cursor = Some(next_cursor);
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

    fn joined_activity_operation(operation_id: &str, tx_hash: &str) -> Value {
        serde_json::json!({
            "id": operation_id,
            "paging_token": operation_id,
            "transaction_hash": tx_hash,
            "type": "set_options",
            "transaction": {
                "hash": tx_hash,
                "ledger": 1234,
                "created_at": "2026-09-07T12:00:00Z",
                "source_account": ADDRESS,
                "fee_account": ADDRESS,
                "fee_charged": "100",
                "max_fee": "100",
                "operation_count": 3,
                "memo_type": "none"
            }
        })
    }

    #[test]
    fn activity_grouping_merges_transaction_split_across_pages() {
        let pages = std::cell::RefCell::new(std::collections::VecDeque::from([
            vec![
                joined_activity_operation("103", "tx-a"),
                joined_activity_operation("102", "tx-a"),
            ],
            vec![
                joined_activity_operation("101", "tx-a"),
                joined_activity_operation("100", "tx-b"),
            ],
        ]));
        let cursors = std::cell::RefCell::new(Vec::new());
        let transactions = collect_activity_transactions(1, 2, |cursor| {
            cursors.borrow_mut().push(cursor.map(str::to_owned));
            Ok(pages.borrow_mut().pop_front().unwrap_or_default())
        })
        .unwrap();

        assert_eq!(cursors.into_inner(), vec![None, Some("102".to_owned())]);
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].transaction_hash, "tx-a");
        assert_eq!(transactions[0].account_operations.len(), 3);
        assert_eq!(
            transactions[0].account_operations[0]
                .operation_id
                .as_deref(),
            Some("101")
        );
        assert_eq!(
            transactions[0].account_operations[2]
                .operation_id
                .as_deref(),
            Some("103")
        );
    }

    #[test]
    fn activity_grouping_confirms_final_group_at_end_of_stream() {
        let pages = std::cell::RefCell::new(std::collections::VecDeque::from([vec![
            joined_activity_operation("201", "tx-only"),
        ]]));
        let transactions = collect_activity_transactions(1, 2, |_| {
            Ok(pages.borrow_mut().pop_front().unwrap_or_default())
        })
        .unwrap();

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].transaction_hash, "tx-only");
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

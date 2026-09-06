from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    if old not in text:
        raise SystemExit(f"expected source fragment not found in {path}: {old[:80]!r}")
    file.write_text(text.replace(old, new, 1))


def replace_all(path: str, old: str, new: str, expected: int) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"expected {expected} matches in {path}, found {count}: {old[:80]!r}")
    file.write_text(text.replace(old, new))


# FresnicaClient explicitly owns every Classic store instead of deriving them through WalletStorage.home().
replace_once(
    "reference/rust-client/src/service.rs",
    """use crate::balance_state::AssetBalance;
use crate::horizon_gateway::{HorizonGateway, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL};
use crate::storage::{WalletRecord, WalletStorage};
""",
    """use crate::asset_catalog::AssetCatalog;
use crate::balance_state::AssetBalance;
use crate::contacts::ContactStore;
use crate::horizon_gateway::{HorizonGateway, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL};
use crate::storage::{WalletRecord, WalletStorage};
use crate::transaction::PendingTransactionStore;
""",
)
replace_once(
    "reference/rust-client/src/service.rs",
    """pub struct FresnicaClient {
    profile: NetworkProfile,
    storage: WalletStorage,
    gateway: HorizonGateway,
}
""",
    """pub struct FresnicaClient {
    profile: NetworkProfile,
    storage: WalletStorage,
    contacts: ContactStore,
    pending_transactions: PendingTransactionStore,
    asset_catalog: AssetCatalog,
    gateway: HorizonGateway,
}
""",
)
replace_once(
    "reference/rust-client/src/service.rs",
    """    pub fn from_profile(home: &Path, profile: NetworkProfile) -> Result<Self, String> {
        let gateway = HorizonGateway::new(profile.horizon_url());
        Ok(Self {
            profile,
            storage: WalletStorage::new(home)?,
            gateway,
        })
    }
""",
    """    pub fn from_profile(home: &Path, profile: NetworkProfile) -> Result<Self, String> {
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
""",
)
replace_once(
    "reference/rust-client/src/service.rs",
    """    pub(crate) fn gateway(&self) -> &HorizonGateway {
        &self.gateway
    }
""",
    """    pub(crate) fn gateway(&self) -> &HorizonGateway {
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
""",
)

# Asset catalog is Client-owned; its provider/cache implementation remains internal.
replace_once(
    "reference/rust-client/src/asset_catalog.rs",
    """struct AssetCatalog {
    network: String,
    path: PathBuf,
}
""",
    """pub(crate) struct AssetCatalog {
    network: String,
    path: PathBuf,
}
""",
)
replace_once(
    "reference/rust-client/src/asset_catalog.rs",
    """        AssetCatalog::new(self.storage().home(), self.network()).load(limit, refresh)
""",
    """        self.asset_catalog_store().load(limit, refresh)
""",
)
replace_once(
    "reference/rust-client/src/asset_catalog.rs",
    """    fn new(home: &Path, network: &str) -> Self {
""",
    """    pub(crate) fn new(home: &Path, network: &str) -> Self {
""",
)

# Destination resolution consumes the explicit contact store, not a wallet filesystem root.
replace_once(
    "reference/rust-client/src/contacts.rs",
    """use crate::storage::WalletStorage;

""",
    "",
)
replace_once(
    "reference/rust-client/src/contacts.rs",
    """pub fn resolve_destination(
    storage: &WalletStorage,
    destination: &str,
    explicit_memo: Option<&str>,
) -> Result<ResolvedDestination, String> {
""",
    """pub fn resolve_destination(
    store: &ContactStore,
    destination: &str,
    explicit_memo: Option<&str>,
) -> Result<ResolvedDestination, String> {
""",
)
replace_once(
    "reference/rust-client/src/contacts.rs",
    """    let store = ContactStore::for_home(storage.home());
    let Some(contact) = store.find(destination)? else {
""",
    """    let Some(contact) = store.find(destination)? else {
""",
)
replace_once(
    "reference/rust-client/src/contacts.rs",
    """        let home = store.path.parent().unwrap().to_path_buf();
        store.add("Alice", ALICE, Some("default-memo")).unwrap();
        let storage = WalletStorage::new(&home).unwrap();

        let resolved = resolve_destination(&storage, "ALICE", None).unwrap();
""",
    """        store.add("Alice", ALICE, Some("default-memo")).unwrap();

        let resolved = resolve_destination(&store, "ALICE", None).unwrap();
""",
)
replace_once(
    "reference/rust-client/src/contacts.rs",
    """        let explicit = resolve_destination(&storage, "alice", Some("explicit")).unwrap();
""",
    """        let explicit = resolve_destination(&store, "alice", Some("explicit")).unwrap();
""",
)
replace_once(
    "reference/rust-client/src/contacts.rs",
    """        let home = store.path.parent().unwrap().to_path_buf();
        store.add(ALICE, BOB, Some("shadowed-memo")).unwrap();
        let storage = WalletStorage::new(&home).unwrap();

        let resolved = resolve_destination(&storage, ALICE, Some("direct-memo")).unwrap();
""",
    """        store.add(ALICE, BOB, Some("shadowed-memo")).unwrap();

        let resolved = resolve_destination(&store, ALICE, Some("direct-memo")).unwrap();
""",
)

# Classic pending-submission safety state is explicitly provided by the Client.
replace_once(
    "reference/rust-client/src/transaction.rs",
    """pub fn resolve_write_wallet(
    storage: &WalletStorage,
    horizon: &HorizonGateway,
""",
    """pub(crate) fn resolve_write_wallet(
    storage: &WalletStorage,
    pending_transactions: &PendingTransactionStore,
    horizon: &HorizonGateway,
""",
)
replace_once(
    "reference/rust-client/src/transaction.rs",
    """    PendingTransactionStore::for_home(storage.home()).reconcile_and_ensure_clear(
        network,
        &record.address,
        horizon,
    )?;
""",
    """    pending_transactions.reconcile_and_ensure_clear(network, &record.address, horizon)?;
""",
)
replace_once(
    "reference/rust-client/src/transaction.rs",
    """pub fn sign_and_submit(
    storage: &WalletStorage,
    record: &WalletRecord,
""",
    """pub(crate) fn sign_and_submit(
    storage: &WalletStorage,
    pending_transactions: &PendingTransactionStore,
    record: &WalletRecord,
""",
)
replace_once(
    "reference/rust-client/src/transaction.rs",
    """            let persist_result = PendingTransactionStore::for_home(storage.home()).remember(
                network,
                &record.address,
                &tx_hash_hex,
                "transaction",
            );
""",
    """            let persist_result = pending_transactions.remember(
                network,
                &record.address,
                &tx_hash_hex,
                "transaction",
            );
""",
)

# Internal transaction helpers are no longer part of the public crate API.
replace_once(
    "reference/rust-client/src/lib.rs",
    """pub use transaction::{
    account_sequence, balance_stroops, build_operation_envelope, build_single_operation_envelope,
    build_single_operation_envelope_with_memo, format_stroops, has_valid_transaction_signature,
    minimum_balance_stroops, network_passphrase, parse_positive_stroops, parse_stroops,
    parse_transaction_xdr, resolve_write_wallet, sign_and_submit,
    sign_transaction_xdr_with_passcode, TransactionSubmission, STROOPS_PER_XLM,
};
""",
    """pub(crate) use transaction::{resolve_write_wallet, sign_and_submit};
pub use transaction::{
    account_sequence, balance_stroops, build_operation_envelope, build_single_operation_envelope,
    build_single_operation_envelope_with_memo, format_stroops, has_valid_transaction_signature,
    minimum_balance_stroops, network_passphrase, parse_positive_stroops, parse_stroops,
    parse_transaction_xdr, sign_transaction_xdr_with_passcode, TransactionSubmission,
    STROOPS_PER_XLM,
};
""",
)

# Payment consumes explicit Client-owned contacts and pending state.
replace_all(
    "reference/rust-client/src/payment.rs",
    """            self.storage(),
            self.gateway(),
            self.network(),
""",
    """            self.storage(),
            self.pending_transaction_store(),
            self.gateway(),
            self.network(),
""",
    2,
)
replace_once(
    "reference/rust-client/src/payment.rs",
    """        let resolved = resolve_destination(
            self.storage(),
            &request.destination,
            request.memo.as_deref(),
        )?;
""",
    """        let resolved = resolve_destination(
            self.contact_store(),
            &request.destination,
            request.memo.as_deref(),
        )?;
""",
)
replace_once(
    "reference/rust-client/src/payment.rs",
    """        sign_and_submit(
            self.storage(),
            &prepared.wallet,
""",
    """        sign_and_submit(
            self.storage(),
            self.pending_transaction_store(),
            &prepared.wallet,
""",
)

# Trustline consumes explicit pending state.
replace_once(
    "reference/rust-client/src/trustline.rs",
    """            self.storage(),
            self.gateway(),
            self.network(),
""",
    """            self.storage(),
            self.pending_transaction_store(),
            self.gateway(),
            self.network(),
""",
)
replace_once(
    "reference/rust-client/src/trustline.rs",
    """        sign_and_submit(
            self.storage(),
            &prepared.wallet,
""",
    """        sign_and_submit(
            self.storage(),
            self.pending_transaction_store(),
            &prepared.wallet,
""",
)

# SDEX consumes explicit pending state for all three write preparation paths and submission.
replace_all(
    "reference/rust-client/src/dex.rs",
    """resolve_write_wallet(self.storage(), self.gateway(), self.network(), wallet_name)?""",
    """resolve_write_wallet(
                self.storage(),
                self.pending_transaction_store(),
                self.gateway(),
                self.network(),
                wallet_name,
            )?""",
    3,
)
replace_once(
    "reference/rust-client/src/dex.rs",
    """        sign_and_submit(
            self.storage(),
            &prepared.wallet,
""",
    """        sign_and_submit(
            self.storage(),
            self.pending_transaction_store(),
            &prepared.wallet,
""",
)

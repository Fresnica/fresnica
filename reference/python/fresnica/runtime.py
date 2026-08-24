"""Fresnica application composition root."""

from dataclasses import dataclass
import os
from pathlib import Path

from .anchor_cache import AnchorCapabilitiesStore
from .anchor_transfer_service import AnchorTransferService
from .asset_catalog import AssetCatalogService
from .balance_service import BalanceService
from .client_system_unlock import UnavailableSystemUnlockBackend
from .contacts import ContactStore
from .datastore import SQLiteDataStore
from .dex_service import DexService
from .history_service import HistoryService
from .manager import WalletManager
from .market_preferences import MarketPreferencesStore
from .network import get_network
from .offer_service import OfferService
from .pending_transactions import PendingTransactionService, PendingTransactionStore
from .settings import SettingsStore
from .stellar_adapter import StellarAdapter
from .storage import FileWalletStorage
from .submit_service import SubmitService
from .testnet import TestnetService
from .transaction_builder_service import TransactionBuilderService
from .transaction_service import TransactionService
from .transfer_service import TransferService
from .trustline_service import TrustlineService


@dataclass
class NetworkServices:
    adapter: StellarAdapter
    balance_service: BalanceService
    history_service: HistoryService
    dex_service: DexService
    offer_service: OfferService
    transaction_builder: TransactionBuilderService
    submit_service: SubmitService
    transaction_service: TransactionService
    pending_transaction_service: PendingTransactionService
    transfer_service: TransferService
    trustline_service: TrustlineService
    testnet_service: TestnetService | None = None


class Runtime:
    def __init__(
        self,
        network: str = "mainnet",
        home: str | Path | None = None,
        wallet_storage=None,
        datastore=None,
        system_unlock_backend=None,
    ):
        self.network = get_network(network).name
        if home is None:
            home = os.environ.get("FRESNICA_HOME", "~/.fresnica")
        self.home = Path(home).expanduser()
        self.home.mkdir(parents=True, exist_ok=True)
        self.wallet_storage = wallet_storage or FileWalletStorage(self.home / "wallets")
        self.datastore = datastore or SQLiteDataStore(self.home / "chain-data.sqlite3")
        self.pending_transaction_store = PendingTransactionStore(
            self.home / "pending-transactions.json"
        )
        self.settings_store = SettingsStore(self.home / "settings.json")
        self.settings = self.settings_store.load()
        self.contact_store = ContactStore(self.home / "contacts.json")
        self.market_preferences = MarketPreferencesStore(self.home / "markets.json")
        self.asset_catalog = AssetCatalogService(self.home / "assets.json")
        self.anchor_capabilities_store = AnchorCapabilitiesStore(self.home / "anchors.json")
        self.anchor_transfer_service = AnchorTransferService()
        self.wallet_manager = WalletManager(self.wallet_storage)
        self.system_unlock_backend = (
            system_unlock_backend or UnavailableSystemUnlockBackend()
        )
        self._services: dict[str, NetworkServices] = {}

    def services_for(self, network_name: str | None = None) -> NetworkServices:
        name = network_name or self.network
        network = get_network(name)
        if network.name not in self._services:
            adapter = StellarAdapter(network)
            balance = BalanceService(adapter, self.datastore, network.name)
            builder = TransactionBuilderService(adapter)
            submit = SubmitService(adapter)
            pending = PendingTransactionService(
                submit.lookup_transaction,
                self.pending_transaction_store,
                network.name,
            )
            transaction = TransactionService(submit, pending)
            services = NetworkServices(
                adapter=adapter,
                balance_service=balance,
                history_service=HistoryService(
                    adapter,
                    self.datastore,
                    network.name,
                    keep_full_history=self.settings.keep_full_history,
                ),
                dex_service=DexService(adapter, self.datastore, network.name),
                offer_service=OfferService(builder, transaction),
                transaction_builder=builder,
                submit_service=submit,
                transaction_service=transaction,
                pending_transaction_service=pending,
                transfer_service=TransferService(balance, builder, transaction),
                trustline_service=TrustlineService(balance, builder, transaction),
            )
            if network.name == "testnet":
                services.testnet_service = TestnetService(adapter)
            self._services[network.name] = services
        return self._services[network.name]

    @property
    def balance_service(self):
        return self.services_for().balance_service

    @property
    def transfer_service(self):
        return self.services_for().transfer_service

    @property
    def transaction_service(self):
        return self.services_for().transaction_service

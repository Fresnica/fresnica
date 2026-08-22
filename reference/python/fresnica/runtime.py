"""Fresnica application composition root."""

from dataclasses import dataclass
import os
from pathlib import Path

from .balance_service import BalanceService
from .datastore import SQLiteDataStore
from .dex_service import DexService
from .history_service import HistoryService
from .manager import WalletManager
from .network import get_network
from .stellar_adapter import StellarAdapter
from .storage import FileWalletStorage
from .submit_service import SubmitService
from .testnet import TestnetService
from .transaction_builder_service import TransactionBuilderService
from .transaction_service import TransactionService
from .transfer_service import TransferService


@dataclass
class NetworkServices:
    adapter: StellarAdapter
    balance_service: BalanceService
    history_service: HistoryService
    dex_service: DexService
    transaction_builder: TransactionBuilderService
    submit_service: SubmitService
    transaction_service: TransactionService
    transfer_service: TransferService
    testnet_service: TestnetService | None = None


class Runtime:
    def __init__(self, network: str = "mainnet", home: str | Path | None = None, wallet_storage=None, datastore=None):
        self.network = get_network(network).name
        if home is None:
            home = os.environ.get("FRESNICA_HOME", "~/.fresnica")
        self.home = Path(home).expanduser()
        self.home.mkdir(parents=True, exist_ok=True)
        self.wallet_storage = wallet_storage or FileWalletStorage(self.home / "wallets")
        self.datastore = datastore or SQLiteDataStore(self.home / "chain-data.sqlite3")
        self.wallet_manager = WalletManager(self.wallet_storage)
        self._services: dict[str, NetworkServices] = {}

    def services_for(self, network_name: str | None = None) -> NetworkServices:
        name = network_name or self.network
        network = get_network(name)
        if network.name not in self._services:
            adapter = StellarAdapter(network)
            balance = BalanceService(adapter, self.datastore, network.name)
            builder = TransactionBuilderService(adapter)
            submit = SubmitService(adapter)
            transaction = TransactionService(submit)
            services = NetworkServices(
                adapter=adapter,
                balance_service=balance,
                history_service=HistoryService(adapter, self.datastore, network.name),
                dex_service=DexService(adapter, self.datastore, network.name),
                transaction_builder=builder,
                submit_service=submit,
                transaction_service=transaction,
                transfer_service=TransferService(balance, builder, transaction),
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

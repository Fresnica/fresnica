"""Fresnica application composition root."""

from dataclasses import dataclass
import os
from pathlib import Path

from .balance_service import BalanceService
from .datastore import SQLiteDataStore
from .history_service import HistoryService
from .manager import WalletManager
from .network import MAINNET, get_network
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
    transaction_builder: TransactionBuilderService
    submit_service: SubmitService
    transaction_service: TransactionService
    transfer_service: TransferService
    testnet_service: TestnetService | None = None


class Runtime:
    def __init__(self, home: str | Path | None = None, wallet_storage=None, datastore=None):
        if home is None:
            home = os.environ.get("FRESNICA_HOME", "~/.fresnica")
        self.home = Path(home).expanduser()
        self.home.mkdir(parents=True, exist_ok=True)
        self.wallet_storage = wallet_storage or FileWalletStorage(self.home / "wallets")
        self.datastore = datastore or SQLiteDataStore(self.home / "chain-data.sqlite3")
        self.wallet_manager = WalletManager(self.wallet_storage)
        self._services: dict[str, NetworkServices] = {}

    def services_for(self, network_name: str) -> NetworkServices:
        network = get_network(network_name)
        if network.name not in self._services:
            adapter = StellarAdapter(network)
            balance_service = BalanceService(adapter, self.datastore, network.name)
            history_service = HistoryService(adapter, self.datastore, network.name)
            builder_service = TransactionBuilderService(adapter)
            submit_service = SubmitService(adapter)
            transaction_service = TransactionService(submit_service)

            services = NetworkServices(
                adapter=adapter,
                balance_service=balance_service,
                history_service=history_service,
                transaction_builder=builder_service,
                submit_service=submit_service,
                transaction_service=transaction_service,
                transfer_service=TransferService(
                    balance_service,
                    builder_service,
                    transaction_service,
                ),
            )

            if network.name == "testnet":
                services.testnet_service = TestnetService(adapter)

            self._services[network.name] = services

        return self._services[network.name]

    @property
    def balance_service(self):
        return self.services_for(MAINNET.name).balance_service

    @property
    def transfer_service(self):
        return self.services_for(MAINNET.name).transfer_service

    @property
    def transaction_service(self):
        return self.services_for(MAINNET.name).transaction_service

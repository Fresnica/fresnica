"""Fresnica runtime composition root.

CLI and TUI receive dependencies from here instead of creating them directly.
"""

from .manager import WalletManager
from .stellar_adapter import StellarAdapter
from .datastore import MemoryDataStore
from .balance_service import BalanceService
from .transfer_service import TransferService
from .transaction_service import TransactionService


class Runtime:
    def __init__(self, horizon_url: str = "https://horizon.stellar.org"):
        self.datastore = MemoryDataStore()
        self.wallet_manager = WalletManager()
        self.stellar = StellarAdapter(horizon_url)

        self.balance_service = BalanceService(
            self.stellar,
            self.datastore,
        )
        self.transaction_service = TransactionService(
            self.stellar,
        )
        self.transfer_service = TransferService(
            self.balance_service,
            self.transaction_service,
        )

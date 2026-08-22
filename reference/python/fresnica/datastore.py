"""Fresnica datastore abstraction.

Datastore stores blockchain-related data separately from wallet identity.

Examples:
- balances
- transactions
- operations
- offers
- trades

Wallet files should only contain identity and signing information.
"""

from abc import ABC, abstractmethod


class DataStore(ABC):
    """Abstract chain data storage."""

    @abstractmethod
    def save_balance(self, account, asset, balance):
        pass

    @abstractmethod
    def get_balances(self, account):
        pass


class MemoryDataStore(DataStore):
    """In-memory datastore for development and testing."""

    def __init__(self):
        self._balances = {}

    def save_balance(self, account, asset, balance):
        self._balances.setdefault(account, {})[asset] = balance

    def get_balances(self, account):
        return self._balances.get(account, {})

"""Fresnica wallet storage abstraction.

Storage manages wallet persistence metadata.
It intentionally does not define encryption yet.
Encryption will be added after the wallet model stabilizes.
"""

from dataclasses import dataclass, field
from typing import Optional


@dataclass
class WalletRecord:
    """Persisted wallet metadata."""

    name: str
    address: str
    wallet_type: str
    metadata: dict = field(default_factory=dict)


class WalletStorage:
    """Abstract wallet storage interface."""

    def save(self, wallet: WalletRecord):
        raise NotImplementedError

    def load(self, name: str) -> Optional[WalletRecord]:
        raise NotImplementedError

    def list(self):
        raise NotImplementedError


class MemoryWalletStorage(WalletStorage):
    """In-memory storage for reference implementation and tests."""

    def __init__(self):
        self._wallets = {}

    def save(self, wallet: WalletRecord):
        self._wallets[wallet.name] = wallet

    def load(self, name: str):
        return self._wallets.get(name)

    def list(self):
        return list(self._wallets.values())

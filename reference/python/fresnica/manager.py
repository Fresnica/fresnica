"""Wallet manager.

Manages wallet lifecycle. It does not replace Wallet.
"""


class WalletManager:
    """User-facing wallet management layer."""

    def __init__(self, storage):
        self.storage = storage
        self._current = None

    def list_wallets(self):
        return self.storage.list()

    def open(self, name: str):
        self._current = self.storage.load(name)
        return self._current

    def current(self):
        return self._current

    def close(self):
        self._current = None

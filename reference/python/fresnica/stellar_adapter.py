"""Stellar network adapter.

This module is a boundary between Fresnica services and Stellar SDK.
It does not replace Stellar SDK functionality.
"""

from stellar_sdk import Server


class StellarAdapter:
    def __init__(self, horizon_url: str):
        self.server = Server(horizon_url)

    def get_account(self, address: str):
        return self.server.accounts().account_id(address).call()

    def get_balances(self, address: str):
        account = self.get_account(address)
        return account.get("balances", [])

    def get_operations(self, address: str, limit: int = 20):
        return (
            self.server.operations()
            .for_account(address)
            .limit(limit)
            .call()
        )

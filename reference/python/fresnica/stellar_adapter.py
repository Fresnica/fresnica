"""Stellar network adapter.

This module is a boundary between Fresnica services and Stellar SDK.
It does not replace Stellar SDK functionality.

Responsibilities:
- provide a stable Fresnica-facing interface
- isolate SDK changes
- prepare data for services
"""

from stellar_sdk import Server


class StellarAdapter:
    def __init__(self, horizon_url: str):
        self.server = Server(horizon_url)

    def get_account(self, address: str):
        """Return raw Stellar SDK account response."""
        return self.server.accounts().account_id(address).call()

    def get_balances(self, address: str):
        """Get account balances.

        Raw SDK response is intentionally preserved at this layer.
        Domain conversion can happen in services when required.
        """
        account = self.get_account(address)
        return account.get("balances", [])

"""Balance and account-state service."""

from .availability import AvailabilityService


class BalanceService:
    def __init__(self, adapter, datastore, network_name: str):
        self.adapter = adapter
        self.datastore = datastore
        self.network_name = network_name
        self.availability = AvailabilityService()

    def get_account(self, wallet, refresh: bool = True) -> dict:
        address = wallet.address()
        if refresh:
            account = self.adapter.get_account(address)
            self.datastore.save_balances(
                self.network_name,
                address,
                account.get("balances", []),
            )
            return account

        return {
            "account_id": address,
            "balances": self.datastore.get_balances(self.network_name, address),
        }

    def get_balances(self, wallet, refresh: bool = True) -> list[dict]:
        return self.get_account(wallet, refresh=refresh).get("balances", [])

    def get_views(self, wallet):
        account = self.get_account(wallet, refresh=True)
        base_reserve = self.adapter.get_base_reserve_stroops()
        return self.availability.balance_views(account, base_reserve)

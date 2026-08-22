"""Balance service.

Combines wallet identity, Stellar adapter data, and datastore caching.

The service keeps raw Stellar data available while providing a wallet-level
entry point for balance queries.
"""


class BalanceService:
    def __init__(self, adapter, datastore=None):
        self.adapter = adapter
        self.datastore = datastore

    def get_balances(self, wallet):
        address = wallet.address()

        data = self.adapter.get_balances(address)

        if self.datastore:
            self.datastore.save_balances(address, data)

        return data

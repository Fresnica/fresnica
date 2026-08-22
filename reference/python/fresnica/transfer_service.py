"""Transfer service.

Builds wallet-level transfer workflow on top of Stellar SDK.

The service coordinates:
- wallet identity
- signer capability
- network adapter
- transaction lifecycle

It does not replace Stellar SDK transaction primitives.
"""


class TransferService:
    def __init__(self, stellar_adapter):
        self.stellar_adapter = stellar_adapter

    def can_transfer(self, wallet) -> bool:
        return wallet.can_sign()

    def check_available_balance(self, balance, amount):
        """Basic available balance check.

        Real Stellar asset availability must consider:
        - balance
        - selling liabilities
        - fees
        - trustline constraints
        """
        available = balance.get("available", balance.get("balance"))
        return float(available) >= float(amount)

    def prepare(self, wallet, destination, asset, amount):
        if not wallet.can_sign():
            raise RuntimeError("Watch-only wallet cannot transfer")

        return {
            "source": wallet.address(),
            "destination": destination,
            "asset": asset,
            "amount": amount,
        }

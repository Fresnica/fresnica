"""Transaction service layer.

Builds the bridge between user intent and Stellar SDK transactions.
Fresnica does not replace Stellar SDK transaction logic.
"""

from dataclasses import dataclass


@dataclass
class TransactionIntent:
    source: str
    destination: str
    asset: str
    amount: str


class TransactionService:
    def __init__(self, adapter):
        self.adapter = adapter

    def prepare(self, intent: TransactionIntent):
        """Prepare a transaction request.

        Transaction building and signing will delegate to Stellar SDK.
        """
        return intent

    def sign(self, wallet, transaction):
        return wallet.sign(transaction)

    def submit(self, transaction):
        return self.adapter.submit_transaction(transaction)

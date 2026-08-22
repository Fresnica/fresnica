"""Build Stellar transactions from user intents.

This layer orchestrates wallet intent and Stellar SDK.
It does not replace Stellar transaction building.
"""


class TransactionBuilderService:
    def __init__(self, adapter):
        self.adapter = adapter

    def build_payment(self, wallet, request):
        if not wallet.can_sign():
            raise RuntimeError("Wallet cannot sign")

        # Future implementation:
        # - load account sequence
        # - build Stellar SDK transaction
        # - return reviewable transaction
        raise NotImplementedError

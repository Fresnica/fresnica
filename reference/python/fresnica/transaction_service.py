"""Generic transaction signing and submission workflow."""

from .models import TransactionResult


class TransactionService:
    def __init__(self, submit_service):
        self.submit_service = submit_service

    def sign(self, wallet, prepared):
        wallet.sign(prepared.envelope)
        return prepared

    def submit(self, prepared) -> TransactionResult:
        response = self.submit_service.submit(prepared.envelope)
        return TransactionResult(
            hash=response.get("hash", ""),
            ledger=response.get("ledger"),
            successful=bool(response.get("successful", True)),
            raw=response,
        )

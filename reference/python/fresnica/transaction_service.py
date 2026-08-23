"""Generic transaction signing and submission workflow."""

from .errors import TransactionSubmissionUncertain
from .models import TransactionResult
from .offer_result import parse_offer_submission_outcome


class TransactionService:
    def __init__(self, submit_service, pending_service=None):
        self.submit_service = submit_service
        self.pending_service = pending_service

    def sign(self, wallet, prepared):
        wallet.sign(prepared.envelope)
        return prepared

    def submit(self, prepared) -> TransactionResult:
        try:
            response = self.submit_service.submit(prepared.envelope)
        except TransactionSubmissionUncertain as exc:
            if self.pending_service is not None:
                review = getattr(prepared, "review", None)
                account = getattr(review, "source", None)
                if account:
                    self.pending_service.remember(
                        account,
                        exc.tx_hash,
                        kind=_pending_kind(review),
                    )
            raise
        return TransactionResult(
            hash=response.get("hash", ""),
            ledger=response.get("ledger"),
            successful=bool(response.get("successful", True)),
            raw=response,
            offer_outcome=parse_offer_submission_outcome(response.get("result_xdr")),
        )


def _pending_kind(review) -> str:
    action = getattr(review, "action", None)
    if action:
        return f"offer:{action}"
    operation = getattr(review, "operation", None)
    return str(operation or "transaction")

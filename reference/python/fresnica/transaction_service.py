"""Generic transaction signing and submission workflow."""

import time

from .errors import TransactionError, TransactionSubmissionUncertain
from .models import TransactionResult
from .offer_result import parse_offer_submission_outcome
from .review import TrustlineReview


class TransactionService:
    def __init__(self, submit_service, pending_service=None):
        self.submit_service = submit_service
        self.pending_service = pending_service

    def sign(self, wallet, prepared):
        _ensure_prepared_transaction_not_expired(prepared)
        assert_review_binding = getattr(prepared, "assert_review_binding", None)
        if assert_review_binding is not None:
            assert_review_binding()
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


def _ensure_prepared_transaction_not_expired(prepared) -> None:
    transaction = getattr(prepared.envelope, "transaction", None)
    preconditions = getattr(transaction, "preconditions", None)
    time_bounds = getattr(preconditions, "time_bounds", None)
    max_time = getattr(time_bounds, "max_time", 0) or 0
    if max_time and int(time.time()) > max_time:
        raise TransactionError(
            "Prepared transaction has expired; prepare and review the transaction again before signing"
        )


def _pending_kind(review) -> str:
    if isinstance(review, TrustlineReview):
        return f"trustline:{review.action}"
    action = getattr(review, "action", None)
    if action:
        return f"offer:{action}"
    operation = getattr(review, "operation", None)
    return str(operation or "transaction")

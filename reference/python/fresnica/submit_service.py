"""Transaction submission and post-submit transaction lookup."""

from stellar_sdk.exceptions import (
    BadResponseError,
    ConnectionError as StellarConnectionError,
    NotFoundError,
    SdkError,
    UnknownRequestError,
)

from .errors import NetworkError, TransactionError, TransactionSubmissionUncertain


class SubmitService:
    def __init__(self, adapter):
        self.adapter = adapter

    def submit(self, signed_transaction):
        try:
            return self.adapter.submit_transaction(signed_transaction)
        except TransactionError as exc:
            cause = exc.__cause__
            if isinstance(
                cause,
                (StellarConnectionError, BadResponseError, UnknownRequestError),
            ):
                raise TransactionSubmissionUncertain(
                    signed_transaction.hash_hex(),
                    details=exc.details,
                ) from exc
            raise

    def lookup_transaction(self, tx_hash: str) -> dict | None:
        """Return a Horizon transaction by hash, or None while it is not found."""
        try:
            return self.adapter.server.transactions().transaction(tx_hash).call()
        except NotFoundError:
            return None
        except SdkError as exc:
            raise NetworkError(
                f"Unable to check Stellar transaction {tx_hash}",
                details=type(exc).__name__,
            ) from exc

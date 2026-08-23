"""Fresnica domain errors."""


class FresnicaError(Exception):
    """Base error for user-facing Fresnica failures."""

    def __init__(self, message: str, details: str | None = None):
        self.details = details
        super().__init__(message)


class WalletError(FresnicaError):
    pass


class WalletNotFoundError(WalletError):
    pass


class WalletExistsError(WalletError):
    pass


class WalletLockedError(WalletError):
    pass


class InvalidPasswordError(WalletError):
    pass


class ProtectionError(WalletError):
    pass


class ProtectionUnavailableError(ProtectionError):
    pass


class SignerError(FresnicaError):
    pass


class WatchOnlyError(SignerError):
    pass


class NetworkError(FresnicaError):
    pass


class TransactionError(FresnicaError):
    pass


class MemoRequiredError(TransactionError):
    """SEP-29 destination requires a transaction memo."""

    def __init__(self, account_id: str):
        self.account_id = account_id
        super().__init__(
            f"Destination {account_id} requires a transaction memo (SEP-29). "
            "Add a memo and try again."
        )


class TransactionSubmissionUncertain(TransactionError):
    """Submission may have reached Stellar but the client did not get a result."""

    def __init__(self, tx_hash: str, details: str | None = None):
        self.tx_hash = tx_hash
        super().__init__(
            f"Transaction submission status is unknown: {tx_hash}",
            details=details,
        )


class TransactionPendingError(TransactionError):
    """A prior uncertain transaction still blocks safe same-account writes."""

    def __init__(self, tx_hash: str):
        self.tx_hash = tx_hash
        super().__init__(
            f"A previous transaction is still pending confirmation: {tx_hash}"
        )


class InvalidAmountError(TransactionError):
    pass


class InvalidAssetError(TransactionError):
    pass


class TrustlineConfirmationRequired(TransactionError):
    def __init__(self, asset: str):
        self.asset = asset
        super().__init__(f"Receiving {asset} requires creating a trustline")


class InsufficientBalanceError(TransactionError):
    def __init__(self, asset: str, requested, available):
        self.asset = asset
        super().__init__(
            f"Insufficient {asset} balance: requested {requested}, available {available}"
        )


class UserCancelled(FresnicaError):
    pass

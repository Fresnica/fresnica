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


class SignerError(FresnicaError):
    pass


class WatchOnlyError(SignerError):
    pass


class NetworkError(FresnicaError):
    pass


class TransactionError(FresnicaError):
    pass


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
        self.requested = requested
        self.available = available
        super().__init__(
            f"Insufficient {asset} balance: requested {requested}, available {available}"
        )


class UserCancelled(FresnicaError):
    pass

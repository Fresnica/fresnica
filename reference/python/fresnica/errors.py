"""Fresnica error definitions.

Errors are separated from Stellar SDK errors so higher layers do not
need to depend directly on SDK exception types.
"""


class FresnicaError(Exception):
    pass


class WalletError(FresnicaError):
    pass


class SignerError(FresnicaError):
    pass


class NetworkError(FresnicaError):
    pass


class TransactionError(FresnicaError):
    pass

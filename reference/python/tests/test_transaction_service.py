from types import SimpleNamespace

import pytest

from fresnica.errors import TransactionError
from fresnica.transaction_service import TransactionService


class RecordingWallet:
    def __init__(self):
        self.signed = False

    def sign(self, envelope):
        self.signed = True


def _prepared(max_time):
    bounds = SimpleNamespace(max_time=max_time)
    preconditions = SimpleNamespace(time_bounds=bounds)
    transaction = SimpleNamespace(preconditions=preconditions)
    envelope = SimpleNamespace(transaction=transaction)
    return SimpleNamespace(envelope=envelope)


def test_sign_rejects_expired_prepared_transaction_before_wallet_sign():
    wallet = RecordingWallet()
    service = TransactionService(submit_service=None)

    with pytest.raises(TransactionError, match="Prepared transaction has expired"):
        service.sign(wallet, _prepared(1))

    assert wallet.signed is False


def test_sign_accepts_unbounded_prepared_transaction():
    wallet = RecordingWallet()
    service = TransactionService(submit_service=None)

    prepared = _prepared(0)
    assert service.sign(wallet, prepared) is prepared
    assert wallet.signed is True

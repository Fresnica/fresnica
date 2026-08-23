from types import SimpleNamespace

import pytest
from stellar_sdk.exceptions import ConnectionError as StellarConnectionError

from fresnica.errors import TransactionError, TransactionSubmissionUncertain
from fresnica.submit_service import SubmitService
from fresnica.transaction_service import TransactionService


class FakeEnvelope:
    def hash_hex(self):
        return "deadbeef"


class UncertainAdapter:
    def submit_transaction(self, signed_transaction):
        try:
            raise StellarConnectionError("offline")
        except StellarConnectionError as cause:
            raise TransactionError("submission failed", details="ConnectionError") from cause


class DefinitiveAdapter:
    def submit_transaction(self, signed_transaction):
        raise TransactionError("tx_bad_seq", details="result_codes=tx_bad_seq")


class RecordingPending:
    def __init__(self):
        self.calls = []

    def remember(self, account, tx_hash, kind="transaction"):
        self.calls.append((account, tx_hash, kind))


def test_connection_failure_becomes_uncertain_with_precomputed_hash():
    service = SubmitService(UncertainAdapter())

    with pytest.raises(TransactionSubmissionUncertain) as exc:
        service.submit(FakeEnvelope())

    assert exc.value.tx_hash == "deadbeef"
    assert exc.value.details == "ConnectionError"


def test_definitive_transaction_error_is_not_reclassified_as_pending():
    service = SubmitService(DefinitiveAdapter())

    with pytest.raises(TransactionError) as exc:
        service.submit(FakeEnvelope())

    assert type(exc.value) is TransactionError
    assert "tx_bad_seq" in str(exc.value)


def test_transaction_service_persists_uncertain_submission_metadata():
    class UncertainSubmit:
        def submit(self, envelope):
            raise TransactionSubmissionUncertain("deadbeef")

    pending = RecordingPending()
    service = TransactionService(UncertainSubmit(), pending)
    prepared = SimpleNamespace(
        envelope=FakeEnvelope(),
        review=SimpleNamespace(source="GACCOUNT", action="create"),
    )

    with pytest.raises(TransactionSubmissionUncertain):
        service.submit(prepared)

    assert pending.calls == [("GACCOUNT", "deadbeef", "offer:create")]

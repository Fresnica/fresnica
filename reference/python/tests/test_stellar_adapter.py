import pytest
from stellar_sdk import Account, Keypair
from stellar_sdk.exceptions import SdkError

from fresnica.errors import TransactionError
from fresnica.models import Asset
from fresnica.network import TESTNET
from fresnica.stellar_adapter import StellarAdapter


class BuildServer:
    def __init__(self, source_address):
        self.source_address = source_address

    def load_account(self, address):
        return Account(account=self.source_address, sequence=1)


class SubmissionError(SdkError):
    status = 400
    title = "Transaction Failed"
    detail = "The transaction failed when submitted to the stellar network."
    extras = {
        "result_codes": {
            "transaction": "tx_failed",
            "operations": ["op_no_destination"],
        }
    }


class FailingServer:
    def submit_transaction(self, transaction):
        raise SubmissionError("failed")


def test_adapter_builds_create_account_operation():
    source = Keypair.random()
    destination = Keypair.random().public_key
    adapter = StellarAdapter(TESTNET)
    adapter.server = BuildServer(source.public_key)

    envelope = adapter.build_payment(
        source=source.public_key,
        destination=destination,
        asset=Asset("XLM"),
        amount="1",
        base_fee=100,
        create_destination=True,
    )

    assert type(envelope.transaction.operations[0]).__name__ == "CreateAccount"


def test_submission_error_preserves_horizon_result_codes_for_developers():
    adapter = StellarAdapter(TESTNET)
    adapter.server = FailingServer()

    with pytest.raises(TransactionError) as captured:
        adapter.submit_transaction(object())

    assert str(captured.value) == "Stellar transaction submission failed"
    assert "status=400" in captured.value.details
    assert "Transaction Failed" in captured.value.details
    assert "op_no_destination" in captured.value.details

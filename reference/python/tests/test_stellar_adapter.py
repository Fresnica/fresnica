import pytest
from stellar_sdk import Account, Keypair
from stellar_sdk.exceptions import SdkError

from fresnica.errors import TransactionError
from fresnica.models import Asset, PriceRatio
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


def test_adapter_builds_manage_sell_and_manage_buy_offer_operations():
    source = Keypair.random()
    issued = Asset("USD", Keypair.random().public_key)
    adapter = StellarAdapter(TESTNET)
    adapter.server = BuildServer(source.public_key)

    sell_envelope = adapter.build_manage_sell_offer(
        source=source.public_key,
        selling=Asset("XLM"),
        buying=issued,
        amount="12.5",
        price="2.4",
        base_fee=100,
        offer_id=7,
    )
    sell = sell_envelope.transaction.operations[0]
    assert type(sell).__name__ == "ManageSellOffer"
    assert str(sell.amount) == "12.5"
    assert sell.offer_id == 7

    buy_envelope = adapter.build_manage_buy_offer(
        source=source.public_key,
        selling=issued,
        buying=Asset("XLM"),
        buy_amount="3",
        price="0.4",
        base_fee=100,
        offer_id=8,
    )
    buy = buy_envelope.transaction.operations[0]
    assert type(buy).__name__ == "ManageBuyOffer"
    assert str(buy.amount) == "3"
    assert buy.offer_id == 8


def test_adapter_places_confirmed_trustline_before_offer():
    source = Keypair.random()
    selling = Asset("XLM")
    buying = Asset("USD", Keypair.random().public_key)
    adapter = StellarAdapter(TESTNET)
    adapter.server = BuildServer(source.public_key)

    envelope = adapter.build_manage_sell_offer(
        source=source.public_key,
        selling=selling,
        buying=buying,
        amount="1",
        price="2",
        base_fee=100,
        trustline_asset=buying,
    )

    assert [type(op).__name__ for op in envelope.transaction.operations] == [
        "ChangeTrust",
        "ManageSellOffer",
    ]


def test_adapter_preserves_exact_price_fraction_for_cancel():
    source = Keypair.random()
    adapter = StellarAdapter(TESTNET)
    adapter.server = BuildServer(source.public_key)

    envelope = adapter.build_manage_sell_offer(
        source=source.public_key,
        selling=Asset("XLM"),
        buying=Asset("USD", Keypair.random().public_key),
        amount="0",
        price=PriceRatio(2000, 337),
        base_fee=100,
        offer_id=99,
    )
    operation = envelope.transaction.operations[0]
    assert operation.price.n == 2000
    assert operation.price.d == 337


def test_submission_error_preserves_horizon_result_codes_for_developers():
    adapter = StellarAdapter(TESTNET)
    adapter.server = FailingServer()

    with pytest.raises(TransactionError) as captured:
        adapter.submit_transaction(object())

    assert str(captured.value) == "Stellar transaction submission failed"
    assert "status=400" in captured.value.details
    assert "Transaction Failed" in captured.value.details
    assert "op_no_destination" in captured.value.details


def test_base_reserve_is_loaded_once_per_adapter_instance():
    adapter = StellarAdapter(TESTNET)
    calls = 0

    def latest_ledger():
        nonlocal calls
        calls += 1
        return {"base_reserve_in_stroops": 5_000_000}

    adapter.get_latest_ledger = latest_ledger

    assert adapter.get_base_reserve_stroops() == 5_000_000
    assert adapter.get_base_reserve_stroops() == 5_000_000
    assert calls == 1

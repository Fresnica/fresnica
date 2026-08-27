from types import SimpleNamespace

import pytest

from fresnica.errors import InsufficientBalanceError, MemoRequiredError, TransactionError
from fresnica.transfer_service import TransferService, _account_requires_memo


SOURCE = "GSOURCE"
DESTINATION = "GDESTINATION"
ISSUER = "GISSUER"


class DummyWallet:
    def can_sign(self):
        return True

    def address(self):
        return SOURCE


class DummyAdapter:
    def __init__(self, source, destination):
        self.source = source
        self.destination = destination

    def account_exists(self, address):
        return address == DESTINATION

    def get_account(self, address):
        if address == SOURCE:
            return self.source
        if address == DESTINATION:
            return self.destination
        raise AssertionError(address)

    def fetch_base_fee(self):
        return 100

    def get_base_reserve_stroops(self):
        return 5_000_000


class DummyBalanceService:
    def __init__(self, adapter):
        self.adapter = adapter

    def get_account(self, wallet, refresh=True):
        assert refresh
        return self.adapter.get_account(wallet.address())


class DummyBuilder:
    def __init__(self):
        self.kwargs = None

    def build_payment(self, **kwargs):
        self.kwargs = kwargs
        return SimpleNamespace(kwargs=kwargs)


class DummyTransactionService:
    pass


def _native(balance="100"):
    return {
        "asset_type": "native",
        "balance": balance,
        "selling_liabilities": "0",
        "buying_liabilities": "0",
    }


def _credit(balance="10", limit="100", buying="0", authorized=True):
    return {
        "asset_type": "credit_alphanum4",
        "asset_code": "USD",
        "asset_issuer": ISSUER,
        "balance": balance,
        "selling_liabilities": "0",
        "buying_liabilities": buying,
        "limit": limit,
        "is_authorized": authorized,
    }


def _account(account_id, balances, data=None):
    return {
        "account_id": account_id,
        "subentry_count": sum(1 for item in balances if item["asset_type"] != "native"),
        "num_sponsoring": 0,
        "num_sponsored": 0,
        "balances": balances,
        "data": data or {},
    }


def _service(source, destination):
    adapter = DummyAdapter(source, destination)
    builder = DummyBuilder()
    return TransferService(
        DummyBalanceService(adapter),
        builder,
        DummyTransactionService(),
    ), builder


def test_sep29_preflight_requires_memo_before_building_transaction():
    source = _account(SOURCE, [_native()])
    destination = _account(
        DESTINATION,
        [_native()],
        data={"config.memo_required": "MQ=="},
    )
    service, builder = _service(source, destination)

    with pytest.raises(MemoRequiredError):
        service.prepare("main", DummyWallet(), DESTINATION, "XLM", "1")
    assert builder.kwargs is None

    service.prepare("main", DummyWallet(), DESTINATION, "XLM", "1", memo="42")
    assert builder.kwargs is not None


def test_destination_credit_requires_full_authorization_and_capacity():
    source = _account(SOURCE, [_native(), _credit(balance="10")])
    destination = _account(
        DESTINATION,
        [_native(), _credit(balance="9.5", limit="10", buying="0.25")],
    )
    service, _ = _service(source, destination)

    service.prepare("main", DummyWallet(), DESTINATION, f"USD:{ISSUER}", "0.25")
    with pytest.raises(InsufficientBalanceError):
        service.prepare("main", DummyWallet(), DESTINATION, f"USD:{ISSUER}", "0.2500001")

    destination["balances"][1]["is_authorized"] = False
    with pytest.raises(TransactionError, match="not fully authorized"):
        service.prepare("main", DummyWallet(), DESTINATION, f"USD:{ISSUER}", "0.1")


def test_source_issuer_can_issue_without_self_trustline():
    source = _account(ISSUER, [_native()])
    destination = _account(DESTINATION, [_native(), _credit(balance="0")])
    adapter = DummyAdapter(source, destination)
    builder = DummyBuilder()
    service = TransferService(
        DummyBalanceService(adapter),
        builder,
        DummyTransactionService(),
    )

    class IssuerWallet(DummyWallet):
        def address(self):
            return ISSUER

    # Route source lookup to the issuer account for this wallet.
    adapter.get_account = lambda address: source if address == ISSUER else destination
    service.prepare("issuer", IssuerWallet(), DESTINATION, f"USD:{ISSUER}", "50")
    assert builder.kwargs["amount"] == 50


def test_memo_required_data_is_exact_ascii_one():
    assert _account_requires_memo({"data": {"config.memo_required": "MQ=="}})
    assert not _account_requires_memo({"data": {"config.memo_required": "MA=="}})
    with pytest.raises(TransactionError):
        _account_requires_memo({"data": {"config.memo_required": "not-base64"}})

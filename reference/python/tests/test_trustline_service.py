from decimal import Decimal
from types import SimpleNamespace

import pytest
from stellar_sdk import Account, Keypair

from fresnica.errors import (
    InsufficientBalanceError,
    InvalidAmountError,
    TransactionError,
    WatchOnlyError,
)
from fresnica.models import Asset
from fresnica.network import TESTNET
from fresnica.review import TrustlineReview
from fresnica.review_presentation import project_review
from fresnica.stellar_adapter import StellarAdapter
from fresnica.transaction_service import _pending_kind
from fresnica.trustline_policy import (
    FRESNICA_TRUSTLINE_LIMIT,
    FRESNICA_TRUSTLINE_LIMIT_TEXT,
)
from fresnica.trustline_service import TrustlineService


class Wallet:
    def __init__(self, address=None, can_sign=True):
        self._address = address or Keypair.random().public_key
        self._can_sign = can_sign

    def address(self):
        return self._address

    def can_sign(self):
        return self._can_sign


class FakeAdapter:
    def __init__(self, base_fee=100, base_reserve=5_000_000):
        self.base_fee = base_fee
        self.base_reserve = base_reserve

    def fetch_base_fee(self):
        return self.base_fee

    def get_base_reserve_stroops(self):
        return self.base_reserve


class FakeBalanceService:
    def __init__(self, account, adapter=None):
        self.account = account
        self.adapter = adapter or FakeAdapter()
        self.refreshes = []

    def get_account(self, wallet, refresh=True):
        self.refreshes.append(refresh)
        return self.account


class FakeBuilder:
    def __init__(self):
        self.calls = []

    def build_trustline(self, **kwargs):
        self.calls.append(kwargs)
        return SimpleNamespace(review="trustline-review", envelope="envelope")


class FakeTransactionService:
    def sign(self, wallet, prepared):
        return prepared

    def submit(self, prepared):
        return "submitted"


class BuildServer:
    def __init__(self, source_address):
        self.source_address = source_address

    def load_account(self, address):
        return Account(account=self.source_address, sequence=1)


def _account(*, native="3", trustline=None, subentries=0):
    balances = [
        {
            "asset_type": "native",
            "balance": native,
            "selling_liabilities": "0",
            "buying_liabilities": "0",
        }
    ]
    if trustline is not None:
        balances.append(trustline)
    return {
        "balances": balances,
        "subentry_count": subentries,
        "num_sponsoring": 0,
        "num_sponsored": 0,
    }


def _line(asset, *, balance="0", buying="0", selling="0", limit="1000"):
    return {
        "asset_type": "credit_alphanum4",
        "asset_code": asset.code,
        "asset_issuer": asset.issuer,
        "balance": balance,
        "buying_liabilities": buying,
        "selling_liabilities": selling,
        "limit": limit,
    }


def _service(account):
    builder = FakeBuilder()
    service = TrustlineService(
        FakeBalanceService(account),
        builder,
        FakeTransactionService(),
    )
    return service, builder


def test_add_requires_new_reserve_and_builds_change_trust():
    wallet = Wallet()
    asset = Asset("USD", Keypair.random().public_key)
    service, builder = _service(_account(native="2"))

    prepared = service.prepare_add("main", wallet, asset, limit="250.5")

    assert prepared.review == "trustline-review"
    call = builder.calls[-1]
    assert call["action"] == "add"
    assert call["asset"] == asset
    assert call["limit"] == Decimal("250.5")
    assert call["base_fee_stroops"] == 100


def test_add_uses_fresnica_marker_limit_by_default():
    wallet = Wallet()
    asset = Asset("USD", Keypair.random().public_key)
    service, builder = _service(_account(native="2"))

    service.prepare_add("main", wallet, asset)

    assert builder.calls[-1]["limit"] == FRESNICA_TRUSTLINE_LIMIT


def test_dex_embedded_change_trust_uses_same_fresnica_marker():
    source = Keypair.random()
    asset = Asset("USD", Keypair.random().public_key)
    adapter = StellarAdapter(TESTNET)
    adapter.server = BuildServer(source.public_key)

    envelope = adapter.build_manage_sell_offer(
        source=source.public_key,
        selling=Asset("XLM"),
        buying=asset,
        amount="1",
        price="1",
        base_fee=100,
        trustline_asset=asset,
    )

    operation = envelope.transaction.operations[0]
    assert type(operation).__name__ == "ChangeTrust"
    assert operation.limit == FRESNICA_TRUSTLINE_LIMIT_TEXT


def test_add_rejects_existing_line_and_insufficient_new_reserve():
    wallet = Wallet()
    asset = Asset("USD", Keypair.random().public_key)

    existing, _ = _service(_account(trustline=_line(asset), subentries=1))
    with pytest.raises(TransactionError, match="already exists"):
        existing.prepare_add("main", wallet, asset)

    poor, _ = _service(_account(native="1.4"))
    with pytest.raises(InsufficientBalanceError):
        poor.prepare_add("main", wallet, asset)


def test_limit_requires_existing_line_and_cannot_drop_below_committed_amount():
    wallet = Wallet()
    asset = Asset("USD", Keypair.random().public_key)

    missing, _ = _service(_account())
    with pytest.raises(TransactionError, match="does not exist"):
        missing.prepare_limit("main", wallet, asset, "10")

    service, builder = _service(
        _account(
            trustline=_line(asset, balance="7", buying="2"),
            subentries=1,
        )
    )
    with pytest.raises(InvalidAmountError, match="balance plus buying liabilities"):
        service.prepare_limit("main", wallet, asset, "8.9999999")

    service.prepare_limit("main", wallet, asset, "9")
    assert builder.calls[-1]["action"] == "limit"
    assert builder.calls[-1]["limit"] == Decimal("9")


def test_remove_requires_zero_balance_and_liabilities_then_uses_zero_limit():
    wallet = Wallet()
    asset = Asset("USD", Keypair.random().public_key)

    blocked, _ = _service(
        _account(trustline=_line(asset, balance="1"), subentries=1)
    )
    with pytest.raises(TransactionError, match="balance or liabilities"):
        blocked.prepare_remove("main", wallet, asset)

    service, builder = _service(_account(trustline=_line(asset), subentries=1))
    service.prepare_remove("main", wallet, asset)
    assert builder.calls[-1]["action"] == "remove"
    assert builder.calls[-1]["limit"] == Decimal("0")


def test_trustline_rejects_watch_only_native_self_issued_and_bad_precision():
    issuer = Keypair.random().public_key
    asset = Asset("USD", issuer)
    service, _ = _service(_account())

    with pytest.raises(WatchOnlyError):
        service.prepare_add("main", Wallet(can_sign=False), asset)
    with pytest.raises(TransactionError, match="native"):
        service.prepare_add("main", Wallet(), Asset("XLM"))
    with pytest.raises(TransactionError, match="own asset"):
        service.prepare_add("main", Wallet(address=issuer), asset)
    with pytest.raises(InvalidAmountError, match="7 decimal"):
        service.prepare_add("main", Wallet(), asset, limit="1.00000001")


def test_adapter_builds_standalone_change_trust_operation():
    source = Keypair.random()
    asset = Asset("USD", Keypair.random().public_key)
    adapter = StellarAdapter(TESTNET)
    adapter.server = BuildServer(source.public_key)

    envelope = adapter.build_change_trust(
        source=source.public_key,
        asset=asset,
        limit="50",
        base_fee=100,
    )

    operation = envelope.transaction.operations[0]
    assert type(operation).__name__ == "ChangeTrust"
    assert operation.asset.code == "USD"
    assert operation.asset.issuer == asset.issuer


def test_trustline_review_uses_shared_projection_and_pending_kind():
    review = TrustlineReview(
        wallet_name="main",
        source="GSOURCE",
        action="remove",
        asset="USD:GISSUER",
        limit="0",
        fee="0.00001",
        network="testnet",
    )

    presentation = project_review(review)

    assert presentation.kind == "trustline"
    assert presentation.title == "Confirm trustline removal"
    assert presentation.summary == "Remove trustline for USD:GISSUER"
    assert presentation.warnings
    assert _pending_kind(review) == "trustline:remove"

from decimal import Decimal

import pytest
from stellar_sdk import Keypair

from fresnica.errors import (
    InsufficientBalanceError,
    InvalidAmountError,
    TransactionError,
    TrustlineConfirmationRequired,
)
from fresnica.models import Asset, MarketPair, OfferIntent, OpenOffer, PriceRatio
from fresnica.network import MAINNET
from fresnica.offer_service import OfferService, offer_view_for_pair, open_offer_from_horizon
from fresnica.submit_service import SubmitService
from fresnica.transaction_builder_service import TransactionBuilderService
from fresnica.transaction_service import TransactionService
from fresnica.wallet import Wallet


class FakeEnvelope:
    def __init__(self):
        self.signatures = []

    def sign(self, keypair):
        self.signatures.append(keypair.public_key)


class FakeAdapter:
    network = MAINNET

    def __init__(self, account=None):
        self.calls = []
        self.account = account

    def fetch_base_fee(self):
        return 100

    def get_base_reserve_stroops(self):
        return 5_000_000

    def get_account(self, address):
        if self.account is None:
            raise AssertionError("test did not configure fresh account state")
        return self.account

    def build_manage_sell_offer(self, **kwargs):
        self.calls.append(("sell", kwargs))
        return FakeEnvelope()

    def build_manage_buy_offer(self, **kwargs):
        self.calls.append(("buy", kwargs))
        return FakeEnvelope()

    def submit_transaction(self, envelope):
        return {"hash": "offer-tx", "ledger": 77, "successful": True}


def _service(adapter):
    builder = TransactionBuilderService(adapter)
    transaction = TransactionService(SubmitService(adapter))
    return OfferService(builder, transaction)


def _pair():
    return MarketPair(
        base=Asset("XRP", Keypair.random().public_key),
        counter=Asset("USDC", Keypair.random().public_key),
    )


def _balance(asset, balance, selling_liabilities="0"):
    if asset.is_native:
        return {
            "asset_type": "native",
            "balance": str(balance),
            "selling_liabilities": str(selling_liabilities),
            "buying_liabilities": "0",
        }
    return {
        "asset_type": "credit_alphanum4" if len(asset.code) <= 4 else "credit_alphanum12",
        "asset_code": asset.code,
        "asset_issuer": asset.issuer,
        "balance": str(balance),
        "selling_liabilities": str(selling_liabilities),
        "buying_liabilities": "0",
        "limit": "922337203685.4775807",
    }


def _account(wallet, balances, subentry_count=None):
    if subentry_count is None:
        subentry_count = sum(1 for item in balances if item["asset_type"] != "native")
    return {
        "account_id": wallet.address(),
        "subentry_count": subentry_count,
        "num_sponsoring": 0,
        "num_sponsored": 0,
        "balances": balances,
    }


def _funded_adapter(wallet, pair):
    return FakeAdapter(
        _account(
            wallet,
            [
                _balance(Asset("XLM"), "100"),
                _balance(pair.base, "1000"),
                _balance(pair.counter, "1000"),
            ],
        )
    )


def test_sell_and_buy_intent_keep_one_human_price_direction():
    pair = _pair()
    wallet = Wallet.from_secret(Keypair.random().secret)
    adapter = _funded_adapter(wallet, pair)
    service = _service(adapter)

    sell = OfferIntent(pair, "sell", Decimal("100"), Decimal("0.325"))
    prepared = service.prepare_create("main", wallet, sell)
    kind, kwargs = adapter.calls[-1]
    assert kind == "sell"
    assert kwargs["selling"] == pair.base
    assert kwargs["buying"] == pair.counter
    assert kwargs["amount"] == "100"
    assert kwargs["price"] == "0.325"
    assert kwargs["offer_id"] == 0
    assert kwargs["trustline_asset"] is None
    assert prepared.review.side == "sell"
    assert prepared.review.base_asset == f"XRP:{pair.base.issuer}"
    assert prepared.review.counter_asset == f"USDC:{pair.counter.issuer}"
    assert prepared.review.total == "32.5"

    buy = OfferIntent(pair, "buy", Decimal("100"), Decimal("0.325"))
    prepared = service.prepare_create("main", wallet, buy)
    kind, kwargs = adapter.calls[-1]
    assert kind == "buy"
    assert kwargs["selling"] == pair.counter
    assert kwargs["buying"] == pair.base
    assert kwargs["buy_amount"] == "100"
    assert kwargs["price"] == "0.325"
    assert prepared.review.side == "buy"


def test_missing_receiving_trustline_requires_explicit_approval():
    pair = _pair()
    wallet = Wallet.from_secret(Keypair.random().secret)
    adapter = FakeAdapter(
        _account(
            wallet,
            [
                _balance(Asset("XLM"), "100"),
                _balance(pair.counter, "1000"),
            ],
        )
    )
    service = _service(adapter)
    intent = OfferIntent(pair, "buy", Decimal("2"), Decimal("1"))

    with pytest.raises(TrustlineConfirmationRequired, match="XRP"):
        service.prepare_create("main", wallet, intent)

    prepared = service.prepare_create(
        "main",
        wallet,
        intent,
        allow_trustline=True,
    )
    kind, kwargs = adapter.calls[-1]
    assert kind == "buy"
    assert kwargs["trustline_asset"] == pair.base
    assert prepared.review.trustline_asset == f"XRP:{pair.base.issuer}"
    assert prepared.review.fee == "0.00002"


def test_issuer_can_sell_own_asset_without_a_trustline_balance():
    wallet = Wallet.from_secret(Keypair.random().secret)
    issued = Asset("ISS", wallet.address())
    pair = MarketPair(base=issued, counter=Asset("XLM"))
    adapter = FakeAdapter(_account(wallet, [_balance(Asset("XLM"), "100")], subentry_count=0))
    service = _service(adapter)

    prepared = service.prepare_create(
        "issuer",
        wallet,
        OfferIntent(pair, "sell", Decimal("1000000"), Decimal("1")),
    )
    kind, kwargs = adapter.calls[-1]
    assert kind == "sell"
    assert kwargs["selling"] == issued
    assert kwargs["trustline_asset"] is None
    assert prepared.review.base_asset == f"ISS:{wallet.address()}"


def test_create_preflight_uses_fresh_selling_liabilities():
    pair = _pair()
    wallet = Wallet.from_secret(Keypair.random().secret)
    adapter = FakeAdapter(
        _account(
            wallet,
            [
                _balance(Asset("XLM"), "100"),
                _balance(pair.base, "2", "1.5"),
                _balance(pair.counter, "100"),
            ],
        )
    )
    service = _service(adapter)

    with pytest.raises(InsufficientBalanceError):
        service.prepare_create(
            "main",
            wallet,
            OfferIntent(pair, "sell", Decimal("1"), Decimal("1")),
        )


def test_buy_preflight_rounds_required_selling_amount_up_to_stroop():
    pair = _pair()
    wallet = Wallet.from_secret(Keypair.random().secret)
    adapter = FakeAdapter(
        _account(
            wallet,
            [
                _balance(Asset("XLM"), "100"),
                _balance(pair.base, "100"),
                _balance(pair.counter, "5.9999999"),
            ],
        )
    )
    service = _service(adapter)

    with pytest.raises(InsufficientBalanceError):
        service.prepare_create(
            "main",
            wallet,
            OfferIntent(pair, "buy", Decimal("2"), Decimal("3")),
        )


def test_credit_offer_preflight_still_requires_xlm_for_new_subentry_and_fee():
    pair = _pair()
    wallet = Wallet.from_secret(Keypair.random().secret)
    balances = [
        _balance(Asset("XLM"), "2.5"),
        _balance(pair.base, "100"),
        _balance(pair.counter, "100"),
    ]
    adapter = FakeAdapter(_account(wallet, balances, subentry_count=2))
    service = _service(adapter)

    with pytest.raises(InsufficientBalanceError) as captured:
        service.prepare_create(
            "main",
            wallet,
            OfferIntent(pair, "sell", Decimal("1"), Decimal("1")),
        )
    assert captured.value.asset == "XLM"

    adapter.account["balances"][0]["balance"] = "2.50001"
    prepared = service.prepare_create(
        "main",
        wallet,
        OfferIntent(pair, "sell", Decimal("1"), Decimal("1")),
    )
    assert prepared.review.fee == "0.00001"


def test_reverse_canonical_offer_projects_to_buy_and_updates_with_manage_buy():
    pair = _pair()
    offer = OpenOffer(
        offer_id="42",
        selling=pair.counter,
        buying=pair.base,
        selling_amount=Decimal("32.5"),
        price_r=PriceRatio(40, 13),
    )
    view = offer_view_for_pair(offer, pair)
    assert view is not None
    assert view.side == "buy"
    assert view.amount == Decimal("100.0000000")
    assert view.price == Decimal("0.3250000")
    assert view.total == Decimal("32.5")

    wallet = Wallet.from_secret(Keypair.random().secret)
    adapter = FakeAdapter()
    service = _service(adapter)
    intent = OfferIntent(pair, "buy", Decimal("90"), Decimal("0.33"))
    service.prepare_update("main", wallet, offer, intent)
    kind, kwargs = adapter.calls[-1]
    assert kind == "buy"
    assert kwargs["selling"] == pair.counter
    assert kwargs["buying"] == pair.base
    assert kwargs["offer_id"] == 42
    assert kwargs["buy_amount"] == "90"
    assert kwargs["price"] == "0.33"


def test_cancel_uses_canonical_manage_sell_and_exact_price_fraction():
    pair = _pair()
    offer = OpenOffer(
        offer_id="77",
        selling=pair.counter,
        buying=pair.base,
        selling_amount=Decimal("10"),
        price_r=PriceRatio(2000, 337),
    )
    wallet = Wallet.from_secret(Keypair.random().secret)
    adapter = FakeAdapter()
    service = _service(adapter)

    service.prepare_cancel("main", wallet, offer)
    kind, kwargs = adapter.calls[-1]
    assert kind == "sell"
    assert kwargs["selling"] == offer.selling
    assert kwargs["buying"] == offer.buying
    assert kwargs["amount"] == "0"
    assert kwargs["price"] == offer.price_r
    assert kwargs["offer_id"] == 77


def test_update_cannot_silently_flip_market_side():
    pair = _pair()
    offer = OpenOffer(
        offer_id="42",
        selling=pair.counter,
        buying=pair.base,
        selling_amount=Decimal("32.5"),
        price_r=PriceRatio(40, 13),
    )
    wallet = Wallet.from_secret(Keypair.random().secret)
    service = _service(FakeAdapter())

    with pytest.raises(TransactionError, match="keep the current market pair"):
        service.prepare_update(
            "main",
            wallet,
            offer,
            OfferIntent(pair, "sell", Decimal("100"), Decimal("0.325")),
        )


def test_offer_values_follow_stellar_seven_decimal_precision():
    pair = _pair()
    wallet = Wallet.from_secret(Keypair.random().secret)
    service = _service(FakeAdapter())

    with pytest.raises(InvalidAmountError, match="7 decimal"):
        service.prepare_create(
            "main",
            wallet,
            OfferIntent(pair, "sell", Decimal("1.00000001"), Decimal("1")),
        )


def test_horizon_offer_parser_preserves_exact_price_fraction():
    pair = _pair()
    raw = {
        "id": "9",
        "seller": Keypair.random().public_key,
        "selling": {
            "asset_type": "credit_alphanum4",
            "asset_code": pair.counter.code,
            "asset_issuer": pair.counter.issuer,
        },
        "buying": {
            "asset_type": "credit_alphanum4",
            "asset_code": pair.base.code,
            "asset_issuer": pair.base.issuer,
        },
        "amount": "42000",
        "price": "0.1666667",
        "price_r": {"n": 1, "d": 6},
    }
    offer = open_offer_from_horizon(raw)
    assert offer.price_r == PriceRatio(1, 6)
    view = offer_view_for_pair(offer, pair)
    assert view is not None
    assert view.side == "buy"
    assert view.amount == Decimal("7000.0000000")
    assert view.price == Decimal("6.0000000")


def test_offer_sign_and_submit_reuse_generic_transaction_pipeline():
    pair = _pair()
    wallet = Wallet.from_secret(Keypair.random().secret)
    adapter = _funded_adapter(wallet, pair)
    service = _service(adapter)
    prepared = service.prepare_create(
        "main",
        wallet,
        OfferIntent(pair, "sell", Decimal("1"), Decimal("2")),
    )

    service.sign(wallet, prepared)
    assert prepared.envelope.signatures == [wallet.address()]
    result = service.submit(prepared)
    assert result.hash == "offer-tx"
    assert result.ledger == 77

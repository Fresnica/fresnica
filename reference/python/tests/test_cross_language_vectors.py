import json
from decimal import Decimal
from pathlib import Path
from types import SimpleNamespace

from fresnica.models import (
    AccountTrade,
    AccountTradeSegment,
    Asset,
    MarketPair,
    OfferIntent,
    OpenOffer,
    PriceRatio,
)
from fresnica.offer_service import (
    _offer_liabilities,
    _stellar_price_ratio,
    canonical_operation,
    offer_view_for_pair,
)
from fresnica.trade_segments import account_trade_segment_for_pair, compress_account_trades
from fresnica.transaction_builder_service import TransactionBuilderService


VECTORS = json.loads(
    (Path(__file__).resolve().parents[3] / "spec/test-vectors/sdex-v1.json").read_text(
        encoding="utf-8"
    )
)


class DummyWallet:
    def address(self):
        return "wallet"


class RecordingAdapter:
    network = SimpleNamespace(name="testnet")

    def __init__(self):
        self.operation = None
        self.kwargs = None

    def build_manage_sell_offer(self, **kwargs):
        self.operation = "manage_sell_offer"
        self.kwargs = kwargs
        return object()

    def build_manage_buy_offer(self, **kwargs):
        self.operation = "manage_buy_offer"
        self.kwargs = kwargs
        return object()


def test_offer_intent_vectors_match_stellar_operation_encoding():
    for case in VECTORS["offer_intents"]:
        pair = _pair(case["pair"])
        intent = OfferIntent(
            pair=pair,
            side=case["side"],
            amount=Decimal(case["amount"]),
            price=Decimal(case["price"]),
        )
        expected = case["expected"]
        adapter = RecordingAdapter()
        builder = TransactionBuilderService(adapter)

        prepared = builder.build_offer(
            wallet_name="wallet",
            wallet=DummyWallet(),
            intent=intent,
            price_r=_stellar_price_ratio(intent.price),
            base_fee_stroops=100,
        )

        assert canonical_operation(intent) == expected["operation"], case["name"]
        assert adapter.operation == expected["operation"], case["name"]
        assert adapter.kwargs["selling"] == _asset(expected["selling"]), case["name"]
        assert adapter.kwargs["buying"] == _asset(expected["buying"]), case["name"]
        amount_key = "buy_amount" if case["side"] == "buy" else "amount"
        assert Decimal(adapter.kwargs[amount_key]) == Decimal(
            expected["operation_amount"]
        ), case["name"]
        assert adapter.kwargs["price"] == _ratio(expected["price_r"]), case["name"]
        assert Decimal(prepared.review.price) == Decimal(
            expected["operation_price"]
        ), case["name"]
        assert (prepared.review.price_n, prepared.review.price_d) == (
            expected["price_r"]["n"],
            expected["price_r"]["d"],
        ), case["name"]


def test_price_rationalization_vectors_match_canonical_ratio():
    for case in VECTORS["price_rationalization"]:
        assert _stellar_price_ratio(Decimal(case["requested"])) == _ratio(
            case["expected"]
        ), case["name"]


def test_offer_liability_vectors_match_stellar_integer_rounding():
    for case in VECTORS["offer_liabilities"]:
        selling, buying = _offer_liabilities(
            case["side"],
            int(Decimal(case["amount"]) * Decimal(10_000_000)),
            _ratio(case["price_r"]),
        )
        assert Decimal(selling) / Decimal(10_000_000) == Decimal(
            case["expected"]["selling"]
        ), case["name"]
        assert Decimal(buying) / Decimal(10_000_000) == Decimal(
            case["expected"]["buying"]
        ), case["name"]


def test_open_offer_projection_vectors():
    for case in VECTORS["offer_projection"]:
        raw = case["offer"]
        offer = OpenOffer(
            offer_id=raw["offer_id"],
            selling=_asset(raw["selling"]),
            buying=_asset(raw["buying"]),
            selling_amount=Decimal(raw["selling_amount"]),
            price_r=_ratio(raw["price_r"]),
        )

        view = offer_view_for_pair(offer, _pair(case["pair"]))

        assert view is not None, case["name"]
        expected = case["expected"]
        assert view.side == expected["side"], case["name"]
        assert view.amount == Decimal(expected["amount"]), case["name"]
        assert view.price == Decimal(expected["price"]), case["name"]
        assert view.total == Decimal(expected["total"]), case["name"]


def test_trade_segment_projection_vectors():
    for case in VECTORS["trade_segment_projection"]:
        source = _segment(case["segment"])

        projected = account_trade_segment_for_pair(source, _pair(case["target_pair"]))

        assert projected is not None, case["name"]
        _assert_segment(projected, case["expected"], case["name"])


def test_trade_compression_vectors():
    for case in VECTORS["trade_compression"]:
        trades = [_trade(raw) for raw in case["trades"]]

        segments = compress_account_trades(trades, case["address"])

        assert len(segments) == len(case["expected"]), case["name"]
        for actual, expected in zip(segments, case["expected"], strict=True):
            _assert_segment(actual, expected, case["name"])


def _asset(raw):
    return Asset(raw["code"], raw.get("issuer"))


def _pair(raw):
    return MarketPair(base=_asset(raw["base"]), counter=_asset(raw["counter"]))


def _ratio(raw):
    return PriceRatio(n=int(raw["n"]), d=int(raw["d"]))


def _trade(raw):
    return AccountTrade(
        trade_id=raw["trade_id"],
        pair=_pair(raw["pair"]),
        base_amount=Decimal(raw["base_amount"]),
        counter_amount=Decimal(raw["counter_amount"]),
        price_r=_ratio(raw["price_r"]),
        side=raw["side"],
        time=None,
        base_account=raw.get("base_account"),
        counter_account=raw.get("counter_account"),
        base_offer_id=raw.get("base_offer_id"),
        counter_offer_id=raw.get("counter_offer_id"),
    )


def _segment(raw):
    return AccountTradeSegment(
        segment_key="implementation-private",
        pair=_pair(raw["pair"]),
        side=raw["side"],
        base_amount=Decimal(raw["base_amount"]),
        counter_amount=Decimal(raw["counter_amount"]),
        price_r=_ratio(raw["price_r"]),
        user_offer_id=raw.get("user_offer_id"),
        trade_count=int(raw["trade_count"]),
        first_time=None,
        last_time=None,
        first_trade_id=raw["first_trade_id"],
        last_trade_id=raw["last_trade_id"],
    )


def _assert_segment(actual, expected, name):
    assert actual.side == expected["side"], name
    assert actual.base_amount == Decimal(expected["base_amount"]), name
    assert actual.counter_amount == Decimal(expected["counter_amount"]), name
    assert actual.price_r == _ratio(expected["price_r"]), name
    assert actual.user_offer_id == expected.get("user_offer_id"), name
    assert actual.trade_count == int(expected["trade_count"]), name
    if "first_trade_id" in expected:
        assert actual.first_trade_id == expected["first_trade_id"], name
    if "last_trade_id" in expected:
        assert actual.last_trade_id == expected["last_trade_id"], name

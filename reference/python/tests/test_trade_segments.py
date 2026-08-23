from decimal import Decimal

from stellar_sdk import Keypair

from fresnica.models import AccountTrade, Asset, MarketPair, PriceRatio
from fresnica.trade_segments import (
    account_trade_for_pair,
    account_trade_segment_for_pair,
    compress_account_trades,
)


ADDRESS = Keypair.random().public_key
PAIR = MarketPair(
    base=Asset("USDC", Keypair.random().public_key),
    counter=Asset("XLM"),
)


def _trade(**overrides):
    values = {
        "trade_id": "t1",
        "pair": PAIR,
        "base_amount": Decimal("10"),
        "counter_amount": Decimal("63.5"),
        "price_r": PriceRatio(127, 20),
        "side": "sell",
        "time": "2026-08-23T00:00:00Z",
        "base_account": ADDRESS,
        "counter_account": Keypair.random().public_key,
        "base_offer_id": "offer-1",
    }
    values.update(overrides)
    return AccountTrade(**values)


def test_same_offer_and_exact_price_merge_despite_amount_ratio_noise():
    records = [
        _trade(
            trade_id="t1",
            base_amount=Decimal("145.8088885"),
            counter_amount=Decimal("925.8864419"),
        ),
        _trade(
            trade_id="t2",
            base_amount=Decimal("0.0094839"),
            counter_amount=Decimal("0.0602228"),
        ),
        _trade(
            trade_id="t3",
            base_amount=Decimal("400"),
            counter_amount=Decimal("2540"),
        ),
    ]

    segments = compress_account_trades(records, ADDRESS)
    assert len(segments) == 1
    assert segments[0].trade_count == 3
    assert segments[0].base_amount == Decimal("545.8183724")
    assert segments[0].counter_amount == Decimal("3465.9466647")
    assert segments[0].price_r == PriceRatio(127, 20)


def test_offer_id_or_price_change_splits_segments():
    records = [
        _trade(trade_id="t1"),
        _trade(trade_id="t2", base_offer_id="offer-2"),
        _trade(trade_id="t3", base_offer_id="offer-2", price_r=PriceRatio(13, 2)),
    ]
    segments = compress_account_trades(records, ADDRESS)
    assert [item.trade_count for item in segments] == [1, 1, 1]


def test_missing_offer_id_is_not_merged():
    records = [
        _trade(trade_id="t1", base_offer_id=None),
        _trade(trade_id="t2", base_offer_id=None),
    ]
    segments = compress_account_trades(records, ADDRESS)
    assert len(segments) == 2


def test_reversed_pair_swaps_amounts_price_accounts_and_offer_ids():
    other = Keypair.random().public_key
    reverse = MarketPair(base=PAIR.counter, counter=PAIR.base)
    trade = _trade(
        base_account=ADDRESS,
        counter_account=other,
        base_offer_id="raw-base",
        counter_offer_id="raw-counter",
        base_amount=Decimal("474.0405734"),
        counter_amount=Decimal("2813.2971715"),
        price_r=PriceRatio(2000, 337),
    )

    normalized = account_trade_for_pair(trade, reverse, ADDRESS)
    assert normalized is not None
    assert normalized.pair == reverse
    assert normalized.base_amount == Decimal("2813.2971715")
    assert normalized.counter_amount == Decimal("474.0405734")
    assert normalized.price_r == PriceRatio(337, 2000)
    assert normalized.side == "buy"
    assert normalized.base_account == other
    assert normalized.counter_account == ADDRESS
    assert normalized.base_offer_id == "raw-counter"
    assert normalized.counter_offer_id == "raw-base"


def test_aggregated_segment_projects_to_selected_reverse_pair():
    segments = compress_account_trades(
        [
            _trade(
                trade_id="t1",
                base_amount=Decimal("474.0405734"),
                counter_amount=Decimal("2813.2971715"),
                price_r=PriceRatio(2000, 337),
            ),
            _trade(
                trade_id="t2",
                base_amount=Decimal("2.7535719"),
                counter_amount=Decimal("16.3416730"),
                price_r=PriceRatio(2000, 337),
            ),
        ],
        ADDRESS,
    )
    reverse = MarketPair(base=PAIR.counter, counter=PAIR.base)

    projected = account_trade_segment_for_pair(segments[0], reverse)
    assert projected is not None
    assert projected.pair == reverse
    assert projected.side == "buy"
    assert projected.base_amount == Decimal("2829.6388445")
    assert projected.counter_amount == Decimal("476.7941453")
    assert projected.price_r == PriceRatio(337, 2000)
    assert projected.user_offer_id == "offer-1"
    assert projected.trade_count == 2

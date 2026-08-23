"""Opt-in live SDEX probe against Stellar Testnet.

This test is skipped during normal CI. Run with:

    FRESNICA_LIVE_TESTNET=1 uv run pytest -q -s tests/test_sdex_testnet_live.py

It creates random disposable Testnet accounts and never prints or persists their
secret keys.
"""

import os
import time
from decimal import Decimal

import pytest
from stellar_sdk import Keypair

from fresnica.datastore import MemoryDataStore
from fresnica.dex_service import DexService
from fresnica.friendbot import FriendbotService
from fresnica.models import Asset, MarketPair, OfferIntent
from fresnica.network import TESTNET
from fresnica.offer_service import OfferService, offer_view_for_pair
from fresnica.stellar_adapter import StellarAdapter
from fresnica.submit_service import SubmitService
from fresnica.trade_segments import account_trade_segment_for_pair
from fresnica.transaction_builder_service import TransactionBuilderService
from fresnica.transaction_service import TransactionService
from fresnica.wallet import Wallet


pytestmark = pytest.mark.skipif(
    os.environ.get("FRESNICA_LIVE_TESTNET") != "1",
    reason="set FRESNICA_LIVE_TESTNET=1 to run the live Testnet probe",
)


def _services():
    adapter = StellarAdapter(TESTNET)
    builder = TransactionBuilderService(adapter)
    transaction = TransactionService(SubmitService(adapter))
    return (
        adapter,
        OfferService(builder, transaction),
        DexService(adapter, MemoryDataStore(), "testnet"),
    )


def _submit_offer(service, wallet, prepared):
    service.sign(wallet, prepared)
    result = service.submit(prepared)
    assert result.successful
    assert result.hash
    return result


def _fund(friendbot, wallet):
    result = friendbot.fund(wallet.address())
    assert result.get("hash")


def _issue_asset(adapter, issuer, destination, asset, amount="100"):
    fee = adapter.fetch_base_fee()
    envelope = adapter.build_payment(
        source=issuer.address(),
        destination=destination.address(),
        asset=asset,
        amount=amount,
        base_fee=fee,
    )
    issuer.sign(envelope)
    response = adapter.submit_transaction(envelope)
    assert response.get("successful", True)


def _matching_open_offer(dex, wallet, pair, side):
    matches = []
    for offer in dex.get_open_offers(wallet, limit=50, refresh=True):
        view = offer_view_for_pair(offer, pair)
        if view is not None and view.side == side:
            matches.append((offer, view))
    assert len(matches) == 1, [
        (offer.offer_id, view.side, str(view.amount), str(view.price))
        for offer, view in matches
    ]
    return matches[0]


def _wait_for_offer_view(dex, wallet, pair, side, amount=None, attempts=12):
    last = None
    for _ in range(attempts):
        try:
            offer, view = _matching_open_offer(dex, wallet, pair, side)
            last = (offer, view)
            if amount is None or view.amount == Decimal(str(amount)):
                return offer, view
        except AssertionError:
            pass
        time.sleep(1)
    if last is not None:
        offer, view = last
        raise AssertionError(
            f"offer {offer.offer_id} remained at amount {view.amount}; expected {amount}"
        )
    raise AssertionError(f"no {side} offer appeared for the selected Testnet pair")


def _ensure_trustline_with_offer(
    offer_service,
    dex,
    wallet,
    issued_asset,
):
    """Exercise Fresnica's explicit ChangeTrust + offer path, then remove setup offer."""
    pair = MarketPair(base=Asset("XLM"), counter=issued_asset)
    prepared = offer_service.prepare_create(
        "probe",
        wallet,
        OfferIntent(
            pair=pair,
            side="sell",
            amount=Decimal("0.1"),
            price=Decimal("1000000"),
        ),
        allow_trustline=True,
    )
    assert prepared.review.trustline_asset is not None
    created = _submit_offer(offer_service, wallet, prepared)
    assert created.offer_outcome is not None
    assert created.offer_outcome.effect == "created"
    assert created.offer_outcome.claimed_offer_count == 0
    assert created.offer_outcome.offer_id
    offer, _ = _wait_for_offer_view(dex, wallet, pair, "sell")
    assert created.offer_outcome.offer_id == offer.offer_id
    cancelled = _submit_offer(
        offer_service,
        wallet,
        offer_service.prepare_cancel("probe", wallet, offer),
    )
    assert cancelled.offer_outcome is not None
    assert cancelled.offer_outcome.effect == "deleted"
    assert cancelled.offer_outcome.claimed_offer_count == 0


def _wait_for_fill_segment(dex, wallet, pair, attempts=12):
    for _ in range(attempts):
        segments = dex.get_account_trade_segments(wallet, limit=200, refresh=True)
        projected = [
            item
            for segment in segments
            if (item := account_trade_segment_for_pair(segment, pair)) is not None
        ]
        if projected:
            matching = [
                item
                for item in projected
                if item.side == "sell"
                and item.trade_count >= 2
                and item.base_amount == Decimal("7")
                and item.counter_amount == Decimal("3.5")
                and Decimal(item.price_r.n) / Decimal(item.price_r.d)
                == Decimal("0.5")
            ]
            if matching:
                return matching[0]
        time.sleep(1)
    raise AssertionError("maker's two Testnet fills did not aggregate into one offer segment")


def test_sdex_write_and_account_fill_probe_on_testnet():
    adapter, offer_service, dex = _services()
    friendbot = FriendbotService(TESTNET.friendbot_url)

    issuer = Wallet.from_secret(Keypair.random().secret)
    maker = Wallet.from_secret(Keypair.random().secret)
    taker = Wallet.from_secret(Keypair.random().secret)

    for wallet in (issuer, maker, taker):
        _fund(friendbot, wallet)

    issued = Asset("FRES", issuer.address())
    pair = MarketPair(base=issued, counter=Asset("XLM"))

    # Use Fresnica itself to create the receiving trustlines. The deliberately
    # extreme setup price prevents these temporary XLM/FRES offers from crossing.
    _ensure_trustline_with_offer(offer_service, dex, maker, issued)
    _ensure_trustline_with_offer(offer_service, dex, taker, issued)
    _issue_asset(adapter, issuer, maker, issued, amount="100")

    # ManageSellOffer: maker posts a unique random-issuer market.
    maker_sell = offer_service.prepare_create(
        "maker",
        maker,
        OfferIntent(
            pair=pair,
            side="sell",
            amount=Decimal("10"),
            price=Decimal("0.5"),
        ),
    )
    assert maker_sell.review.side == "sell"
    maker_created = _submit_offer(offer_service, maker, maker_sell)
    assert maker_created.offer_outcome is not None
    assert maker_created.offer_outcome.effect == "created"
    assert maker_created.offer_outcome.claimed_offer_count == 0
    assert maker_created.offer_outcome.offer_id
    maker_offer, maker_view = _wait_for_offer_view(dex, maker, pair, "sell", amount="10")
    assert maker_created.offer_outcome.offer_id == maker_offer.offer_id
    assert maker_view.price == Decimal("0.5000000")

    # Two separate ManageBuyOffer transactions cross the same maker offer. This
    # creates two Horizon trade records carrying the same maker offer id/price.
    for amount in ("4", "3"):
        prepared = offer_service.prepare_create(
            "taker",
            taker,
            OfferIntent(
                pair=pair,
                side="buy",
                amount=Decimal(amount),
                price=Decimal("0.6"),
            ),
        )
        assert prepared.review.side == "buy"
        crossed = _submit_offer(offer_service, taker, prepared)
        assert crossed.offer_outcome is not None
        assert crossed.offer_outcome.effect == "deleted"
        assert crossed.offer_outcome.claimed_offer_count == 1
        assert crossed.offer_outcome.offer_id is None

    maker_offer, maker_view = _wait_for_offer_view(dex, maker, pair, "sell", amount="3")
    assert maker_view.price == Decimal("0.5000000")
    segment = _wait_for_fill_segment(dex, maker, pair)
    assert segment.user_offer_id == maker_offer.offer_id

    # Remove the partially-filled ManageSell offer before probing a resting BUY.
    maker_cancelled = _submit_offer(
        offer_service,
        maker,
        offer_service.prepare_cancel("maker", maker, maker_offer),
    )
    assert maker_cancelled.offer_outcome is not None
    assert maker_cancelled.offer_outcome.effect == "deleted"

    # ManageBuyOffer: use a non-crossing price so the BUY remains on the ledger,
    # then update it through the same pair-relative BUY intent and cancel it.
    buy_prepared = offer_service.prepare_create(
        "taker",
        taker,
        OfferIntent(
            pair=pair,
            side="buy",
            amount=Decimal("2"),
            price=Decimal("0.1"),
        ),
    )
    buy_created = _submit_offer(offer_service, taker, buy_prepared)
    assert buy_created.offer_outcome is not None
    assert buy_created.offer_outcome.effect == "created"
    assert buy_created.offer_outcome.claimed_offer_count == 0
    assert buy_created.offer_outcome.offer_id
    buy_offer, buy_view = _wait_for_offer_view(dex, taker, pair, "buy", amount="2")
    assert buy_created.offer_outcome.offer_id == buy_offer.offer_id
    assert buy_view.price == Decimal("0.1000000")

    updated = offer_service.prepare_update(
        "taker",
        taker,
        buy_offer,
        OfferIntent(
            pair=pair,
            side="buy",
            amount=Decimal("3"),
            price=Decimal("0.11"),
        ),
    )
    assert updated.review.side == "buy"
    buy_updated = _submit_offer(offer_service, taker, updated)
    assert buy_updated.offer_outcome is not None
    assert buy_updated.offer_outcome.effect == "updated"
    assert buy_updated.offer_outcome.offer_id == buy_offer.offer_id
    buy_offer, buy_view = _wait_for_offer_view(dex, taker, pair, "buy", amount="3")
    assert buy_view.price == Decimal("0.1100000")

    buy_cancelled = _submit_offer(
        offer_service,
        taker,
        offer_service.prepare_cancel("taker", taker, buy_offer),
    )
    assert buy_cancelled.offer_outcome is not None
    assert buy_cancelled.offer_outcome.effect == "deleted"
    assert dex.get_open_offers(taker, limit=50, refresh=True) == []

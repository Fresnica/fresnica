from stellar_sdk import Keypair

from fresnica.models import Asset, MarketPair
from fresnica.tui.dex import DexScreen


def test_dex_status_marks_partial_fill_sync_until_next_refresh():
    pair = MarketPair(Asset("USD", Keypair.random().public_key), Asset("XLM"))
    screen = DexScreen(object(), pair, lambda *args: None)
    screen._counts = (20, 20, 30, 4, 10)
    screen._fills_caught_up = False
    messages = []
    screen.set_status = messages.append

    screen._set_market_status("snapshot loaded")

    assert "10 fill segments" in messages[-1]
    assert "fill sync partial" in messages[-1]
    assert "R continue" in messages[-1]

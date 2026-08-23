from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace(path, old, new):
    file = ROOT / path
    text = file.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"missing patch2 target in {path}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# Existing DEX screen test now inspects the full-width Rich orderbook grid.
replace(
    "reference/python/tests/test_tui_dex.py",
    "from fresnica.tui.dex import DexScreen, MarketPairDialog, OfferFormDialog\n",
    "from fresnica.tui.dex import DexScreen, MarketPairDialog, OfferFormDialog, _orderbook_grid\n",
)
replace(
    "reference/python/tests/test_tui_dex.py",
    '''            asks = app.screen.query_one("#dex-asks", DataTable)\n            bids = app.screen.query_one("#dex-bids", DataTable)\n            assert asks.row_count == 1\n            assert bids.row_count == 1\n            assert _plain_row(bids, 0) == ["625.0000000", "0.3200000"]\n            assert _plain_row(asks, 0) == ["0.3300000", "100.0000000"]\n''',
    '''            asks = app.screen.query_one("#dex-asks", Static)\n            bids = app.screen.query_one("#dex-bids", Static)\n            assert asks is not None and bids is not None\n            bid_grid = _orderbook_grid(app.screen._orderbook["bids"], "bid")\n            ask_grid = _orderbook_grid(app.screen._orderbook["asks"], "ask")\n            assert bid_grid.columns[0]._cells[1].plain == "625.0000000"\n            assert bid_grid.columns[1]._cells[1].plain == "0.3200000"\n            assert ask_grid.columns[0]._cells[1].plain == "0.3300000"\n            assert ask_grid.columns[1]._cells[1].plain == "100.0000000"\n''',
)

# Use the pure Rich-grid helper rather than Textual's RichVisual wrapper in the
# focused market UX test.
replace(
    "reference/python/tests/test_tui_dex_market_ux.py",
    "from fresnica.tui.dex import DexScreen, MarketPairDialog\n",
    "from fresnica.tui.dex import DexScreen, MarketPairDialog, _orderbook_grid\n",
)
replace(
    "reference/python/tests/test_tui_dex_market_ux.py",
    '''            bid_grid = bids.render()\n            ask_grid = asks.render()\n''',
    '''            bid_grid = _orderbook_grid(app.screen._orderbook["bids"], "bid")\n            ask_grid = _orderbook_grid(app.screen._orderbook["asks"], "ask")\n''',
)

# The fake trustline pipeline should now observe and review the visible marker.
replace(
    "reference/python/tests/test_tui_trustlines.py",
    "from fresnica.tui.trustlines import TrustlineFormDialog, TrustlineScreen\n",
    "from fresnica.trustline_policy import FRESNICA_TRUSTLINE_LIMIT_TEXT\nfrom fresnica.tui.trustlines import TrustlineFormDialog, TrustlineScreen\n",
)
replace(
    "reference/python/tests/test_tui_trustlines.py",
    '''            assert isinstance(app.screen, TrustlineFormDialog)\n            assert str(app.screen.query_one("#asset-label", Static).render()) == runtime.recommended_asset\n            await pilot.click("#review")\n''',
    '''            assert isinstance(app.screen, TrustlineFormDialog)\n            assert str(app.screen.query_one("#asset-label", Static).render()) == runtime.recommended_asset\n            assert app.screen.query_one("#limit", Input).value == FRESNICA_TRUSTLINE_LIMIT_TEXT\n            await pilot.click("#review")\n''',
)
replace(
    "reference/python/tests/test_tui_trustlines.py",
    '''            assert runtime.trustline_service.calls == [("add", runtime.recommended_asset, None)]\n            assert isinstance(app.screen, ReviewPresentationDialog)\n            text = str(app.screen.query_one("#review-text", Static).render())\n            assert f"Add trustline for {runtime.recommended_asset}" in text\n            assert "Limit: Stellar maximum" in text\n''',
    '''            assert runtime.trustline_service.calls == [\n                ("add", runtime.recommended_asset, FRESNICA_TRUSTLINE_LIMIT_TEXT)\n            ]\n            assert isinstance(app.screen, ReviewPresentationDialog)\n            text = str(app.screen.query_one("#review-text", Static).render())\n            assert f"Add trustline for {runtime.recommended_asset}" in text\n            assert f"Limit: {FRESNICA_TRUSTLINE_LIMIT_TEXT}" in text\n''',
)

print("patch2 applied")

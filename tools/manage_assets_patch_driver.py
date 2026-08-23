from pathlib import Path
import runpy

root = Path(__file__).resolve().parents[1]
python_root = root / "reference" / "python"
py = python_root / "fresnica"
already = (
    "ASSET_LIST_CATALOGUE" in (py / "asset_catalog.py").read_text(encoding="utf-8")
    and 'yield Static("Manage Assets"' in (py / "tui" / "trustlines.py").read_text(encoding="utf-8")
    and "self.set_focus(offers)" in (py / "tui" / "dex.py").read_text(encoding="utf-8")
)
if already:
    print("manage-assets patch already applied")
else:
    try:
        runpy.run_path(str(root / "tools" / "manage_assets_patch.py"), run_name="__main__")
    except SystemExit as exc:
        message = str(exc)
        if "test_tui_dex_market_ux.py" not in message:
            raise

        dex_test = python_root / "tests" / "test_tui_dex_market_ux.py"
        text = dex_test.read_text(encoding="utf-8")
        old = '            fills = app.screen.query_one("#dex-fills", DataTable)\n\n            bid_grid = _orderbook_grid(app.screen._orderbook["bids"], "bid")\n'
        new = '            fills = app.screen.query_one("#dex-fills", DataTable)\n            offers = app.screen.query_one("#dex-offers", DataTable)\n            assert app.screen.focused is offers\n\n            bid_grid = _orderbook_grid(app.screen._orderbook["bids"], "bid")\n'
        if old not in text:
            raise SystemExit("updated DEX test pattern not found")
        dex_test.write_text(text.replace(old, new, 1), encoding="utf-8")

        trust_test = python_root / "tests" / "test_tui_trustlines.py"
        text = trust_test.read_text(encoding="utf-8")
        old = '            table = app.screen.query_one("#trustlines", DataTable)\n            assert table.row_count == 1\n'
        new = '            assert str(app.screen.query_one("#trust-title", Static).render()) == "Manage Assets"\n            assert "E set limit" not in str(app.screen.query_one("#trust-status", Static).render())\n            table = app.screen.query_one("#trustlines", DataTable)\n            assert table.row_count == 1\n'
        if old not in text:
            raise SystemExit("trustline UI test pattern not found")
        trust_test.write_text(text.replace(old, new, 1), encoding="utf-8")
        print("manage-assets patch applied with evolved test recovery")

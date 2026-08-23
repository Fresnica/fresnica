from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / "reference" / "python"


def replace(path, old, new):
    file = ROOT / path
    text = file.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"missing patch target in {path}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# Product-visible trustline marker policy.
policy = PY / "fresnica" / "trustline_policy.py"
policy.write_text(
    '''"""Fresnica trustline defaults that are intentionally visible on-chain."""\n\nfrom decimal import Decimal\n\n\n# 708269837873.6765 encodes the FRESNICA product marker while remaining\n# below Stellar's maximum trustline limit. The value is shown to users in the\n# trustline form/review; users may replace it with another valid limit.\nFRESNICA_TRUSTLINE_LIMIT_TEXT = "708269837873.6765"\nFRESNICA_TRUSTLINE_LIMIT = Decimal(FRESNICA_TRUSTLINE_LIMIT_TEXT)\n''',
    encoding="utf-8",
)

# Explicit trustline creation defaults to the Fresnica marker.
replace(
    "reference/python/fresnica/trustline_service.py",
    "from .models import Asset\n",
    "from .models import Asset\nfrom .trustline_policy import FRESNICA_TRUSTLINE_LIMIT\n",
)
replace(
    "reference/python/fresnica/trustline_service.py",
    "        limit_value = _limit(limit) if limit is not None else None\n",
    "        limit_value = (\n            _limit(limit) if limit is not None else FRESNICA_TRUSTLINE_LIMIT\n        )\n",
)

# The Add Trustline UI displays the marker rather than silently using it.
replace(
    "reference/python/fresnica/tui/trustlines.py",
    "from ..presentation import format_amount\n",
    "from ..presentation import format_amount\nfrom ..trustline_policy import FRESNICA_TRUSTLINE_LIMIT_TEXT\n",
)
replace(
    "reference/python/fresnica/tui/trustlines.py",
    "        self.kind = kind\n        self.asset = asset\n        self.limit = limit or \"\"\n",
    "        self.kind = kind\n        self.asset = asset\n        self.limit = (\n            limit\n            if limit is not None\n            else (FRESNICA_TRUSTLINE_LIMIT_TEXT if kind == \"add\" else \"\")\n        )\n",
)
replace(
    "reference/python/fresnica/tui/trustlines.py",
    '                    "Limit (blank = Stellar maximum)"\n                    if self.kind == "add"\n',
    '                    "Trustline limit (Fresnica default shown)"\n                    if self.kind == "add"\n',
)

# DEX atomic trustline creation uses the same visible marker.
replace(
    "reference/python/fresnica/stellar_adapter.py",
    "from .models import Asset, PriceRatio\n",
    "from .models import Asset, PriceRatio\nfrom .trustline_policy import FRESNICA_TRUSTLINE_LIMIT_TEXT\n",
)
replace(
    "reference/python/fresnica/stellar_adapter.py",
    "                builder = builder.append_change_trust_op(\n                    asset=self.to_sdk_asset(trustline_asset)\n                )\n",
    "                builder = builder.append_change_trust_op(\n                    asset=self.to_sdk_asset(trustline_asset),\n                    limit=FRESNICA_TRUSTLINE_LIMIT_TEXT,\n                )\n",
)

# Tell the user about the marker when DEX needs an atomic trustline.
replace(
    "reference/python/fresnica/tui/app.py",
    "from ..presentation import format_timestamp, offer_outcome_summary\n",
    "from ..presentation import format_timestamp, offer_outcome_summary\nfrom ..trustline_policy import FRESNICA_TRUSTLINE_LIMIT_TEXT\n",
)
replace(
    "reference/python/fresnica/tui/app.py",
    '                f"This offer requires a new trustline for {identity}. "\n                "Fresnica will submit ChangeTrust and the offer in one transaction.",\n',
    '                f"This offer requires a new trustline for {identity}. "\n                f"Fresnica will use trustline limit {FRESNICA_TRUSTLINE_LIMIT_TEXT} "\n                "and submit ChangeTrust with the offer in one transaction.",\n',
)

# DEX market picker: focus the market list immediately, and make F clearly a list switch.
replace(
    "reference/python/fresnica/tui/dex.py",
    "from rich.text import Text\n",
    "from rich.table import Table\nfrom rich.text import Text\n",
)
replace(
    "reference/python/fresnica/tui/dex.py",
    '        Binding("f", "favorites", "Starred"),\n',
    '        Binding("f", "favorites", "Favorites list"),\n',
)
replace(
    "reference/python/fresnica/tui/dex.py",
    '                yield Button(Text("★ Starred [F]"), id="favorites")\n',
    '                yield Button(Text("★ Favorites [F]"), id="favorites")\n',
)
replace(
    "reference/python/fresnica/tui/dex.py",
    "        self._render_market_list()\n        self._load_popular()\n",
    "        self._render_market_list()\n        self.call_later(lambda: self.set_focus(table))\n        self._load_popular()\n",
)

# Order book is read-only presentation, so use full-width Rich grids rather than
# narrow focusable DataTables. This keeps BID prices against the center spread.
replace(
    "reference/python/fresnica/tui/dex.py",
    "    .book-pane { width: 1fr; height: 1fr; padding: 0 1; }\n    #bids-pane { align-horizontal: right; }\n    .bid-section { text-align: right; }\n    #dex-bids { width: auto; min-width: 34; }\n    #dex-asks, #dex-bids { height: 1fr; }\n",
    "    .book-pane { width: 1fr; height: 1fr; padding: 0 1; }\n    .bid-section { width: 100%; text-align: right; }\n    #dex-asks, #dex-bids { width: 100%; height: 1fr; overflow-y: auto; }\n",
)
replace(
    "reference/python/fresnica/tui/dex.py",
    '                yield Label("BID · BUY", classes="dex-section bid-section")\n                yield DataTable(id="dex-bids")\n            with Vertical(id="asks-pane", classes="book-pane"):\n                yield Label("ASK · SELL", classes="dex-section")\n                yield DataTable(id="dex-asks")\n',
    '                yield Label("BID · BUY", classes="dex-section bid-section")\n                yield Static("", id="dex-bids")\n            with Vertical(id="asks-pane", classes="book-pane"):\n                yield Label("ASK · SELL", classes="dex-section")\n                yield Static("", id="dex-asks")\n',
)
replace(
    "reference/python/fresnica/tui/dex.py",
    '    def on_mount(self) -> None:\n        bids = self.query_one("#dex-bids", DataTable)\n        bids.add_columns("Amount", "Price")\n        bids.cursor_type = "row"\n        asks = self.query_one("#dex-asks", DataTable)\n        asks.add_columns("Price", "Amount")\n        asks.cursor_type = "row"\n        trades = self.query_one("#dex-trades", DataTable)\n',
    '    def on_mount(self) -> None:\n        trades = self.query_one("#dex-trades", DataTable)\n',
)
replace(
    "reference/python/fresnica/tui/dex.py",
    '''    def _render_orderbook(self, orderbook: dict) -> None:\n        asks = self.query_one("#dex-asks", DataTable)\n        bids = self.query_one("#dex-bids", DataTable)\n        asks.clear()\n        bids.clear()\n        for row in orderbook.get("asks", []):\n            amount = _decimal(row.get("amount", "0"))\n            asks.add_row(\n                _stellar_decimal_text(_book_price(row), style="red"),\n                _stellar_decimal_text(amount),\n            )\n        for row in orderbook.get("bids", []):\n            amount = _bid_base_amount(row)\n            bids.add_row(\n                _stellar_decimal_text(amount, justify="right"),\n                _stellar_decimal_text(_book_price(row), style="green", justify="right"),\n            )\n''',
    '''    def _render_orderbook(self, orderbook: dict) -> None:\n        self.query_one("#dex-bids", Static).update(\n            _orderbook_grid(orderbook.get("bids", []), "bid")\n        )\n        self.query_one("#dex-asks", Static).update(\n            _orderbook_grid(orderbook.get("asks", []), "ask")\n        )\n''',
)
replace(
    "reference/python/fresnica/tui/dex.py",
    '''    def _clear_market_tables(self) -> None:\n        for selector in (\n            "#dex-asks",\n            "#dex-bids",\n            "#dex-trades",\n            "#dex-offers",\n            "#dex-fills",\n        ):\n            if self.query(selector):\n                self.query_one(selector, DataTable).clear()\n''',
    '''    def _clear_market_tables(self) -> None:\n        for selector in ("#dex-asks", "#dex-bids"):\n            if self.query(selector):\n                self.query_one(selector, Static).update("")\n        for selector in ("#dex-trades", "#dex-offers", "#dex-fills"):\n            if self.query(selector):\n                self.query_one(selector, DataTable).clear()\n''',
)
insert_target = '''def _trade_price(raw: dict) -> Decimal:\n'''
helper = '''def _orderbook_grid(rows, side: Literal["bid", "ask"]) -> Table:\n    """Render a full-width, non-focusable order-book side.\n\n    BID uses Amount | Price with both cells right-aligned so the bid price hugs\n    the center spread. ASK mirrors it as Price | Amount from the center outward.\n    """\n    table = Table.grid(expand=True, padding=(0, 1))\n    if side == "bid":\n        table.add_column(justify="right", ratio=1)\n        table.add_column(justify="right", ratio=1)\n        table.add_row(Text("Amount", style="bold dim"), Text("Price", style="bold dim"))\n        for row in rows:\n            table.add_row(\n                _stellar_decimal_text(_bid_base_amount(row)),\n                _stellar_decimal_text(_book_price(row), style="green"),\n            )\n        return table\n\n    table.add_column(justify="left", ratio=1)\n    table.add_column(justify="left", ratio=1)\n    table.add_row(Text("Price", style="bold dim"), Text("Amount", style="bold dim"))\n    for row in rows:\n        table.add_row(\n            _stellar_decimal_text(_book_price(row), style="red"),\n            _stellar_decimal_text(_decimal(row.get("amount", "0"))),\n        )\n    return table\n\n\n'''
replace(
    "reference/python/fresnica/tui/dex.py",
    insert_target,
    helper + insert_target,
)

# Tests: marker policy, atomic DEX trustline, picker focus, and orderbook alignment.
replace(
    "reference/python/tests/test_trustline_service.py",
    "from fresnica.trustline_service import TrustlineService\n",
    "from fresnica.trustline_policy import (\n    FRESNICA_TRUSTLINE_LIMIT,\n    FRESNICA_TRUSTLINE_LIMIT_TEXT,\n)\nfrom fresnica.trustline_service import TrustlineService\n",
)
anchor = '''def test_add_rejects_existing_line_and_insufficient_new_reserve():\n'''
new_test = '''def test_add_uses_fresnica_marker_limit_by_default():\n    wallet = Wallet()\n    asset = Asset("USD", Keypair.random().public_key)\n    service, builder = _service(_account(native="2"))\n\n    service.prepare_add("main", wallet, asset)\n\n    assert builder.calls[-1]["limit"] == FRESNICA_TRUSTLINE_LIMIT\n\n\ndef test_dex_embedded_change_trust_uses_same_fresnica_marker():\n    source = Keypair.random()\n    asset = Asset("USD", Keypair.random().public_key)\n    adapter = StellarAdapter(TESTNET)\n    adapter.server = BuildServer(source.public_key)\n\n    envelope = adapter.build_manage_sell_offer(\n        source=source.public_key,\n        selling=Asset("XLM"),\n        buying=asset,\n        amount="1",\n        price="1",\n        base_fee=100,\n        trustline_asset=asset,\n    )\n\n    operation = envelope.transaction.operations[0]\n    assert type(operation).__name__ == "ChangeTrust"\n    assert operation.limit == FRESNICA_TRUSTLINE_LIMIT_TEXT\n\n\n'''
replace(
    "reference/python/tests/test_trustline_service.py",
    anchor,
    new_test + anchor,
)

# Adapt the end-to-end DEX TUI assertions from DataTable orderbooks to Rich grids.
replace(
    "reference/python/tests/test_tui_dex_market_ux.py",
    '            markets = app.screen.query_one("#market-list", DataTable)\n\n            # Popular follows Fex\'s held-asset ordering:',
    '            markets = app.screen.query_one("#market-list", DataTable)\n            assert app.screen.focused is markets\n            assert app.screen.query_one("#favorites", Button).label.plain == "★ Favorites [F]"\n\n            # Popular follows Fex\'s held-asset ordering:',
)
replace(
    "reference/python/tests/test_tui_dex_market_ux.py",
    '            asks = app.screen.query_one("#dex-asks", DataTable)\n            bids = app.screen.query_one("#dex-bids", DataTable)\n',
    '            asks = app.screen.query_one("#dex-asks", Static)\n            bids = app.screen.query_one("#dex-bids", Static)\n',
)
replace(
    "reference/python/tests/test_tui_dex_market_ux.py",
    '''            assert [str(column.label) for column in bids.columns.values()] == ["Amount", "Price"]\n            assert [str(column.label) for column in asks.columns.values()] == ["Price", "Amount"]\n            assert [str(column.label) for column in trades.columns.values()] == ["Price", "Amount", "Time (UTC)"]\n\n            # SSE replaces the REST book. Bid raw amount is quote amount:\n            # 4.1 XLM / 0.41 XLM/XRP = 10 XRP BASE amount.\n            assert _plain_row(bids, 0) == ["10.0000000", "0.4100000"]\n            assert _plain_row(asks, 0) == ["0.5100000", "20.0000000"]\n''',
    '''            bid_grid = bids.render()\n            ask_grid = asks.render()\n            assert [column.justify for column in bid_grid.columns] == ["right", "right"]\n            assert [column.justify for column in ask_grid.columns] == ["left", "left"]\n            assert [str(column.label) for column in trades.columns.values()] == ["Price", "Amount", "Time (UTC)"]\n\n            # SSE replaces the REST book. Bid raw amount is quote amount:\n            # 4.1 XLM / 0.41 XLM/XRP = 10 XRP BASE amount. Header is cell 0.\n            assert bid_grid.columns[0]._cells[1].plain == "10.0000000"\n            assert bid_grid.columns[1]._cells[1].plain == "0.4100000"\n            assert ask_grid.columns[0]._cells[1].plain == "0.5100000"\n            assert ask_grid.columns[1]._cells[1].plain == "20.0000000"\n''',
)

print("patch applied")

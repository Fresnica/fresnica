from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


def lines(*items: str) -> str:
    return "\n".join(items)


path = Path("reference/python/fresnica/tui/dex.py")
text = path.read_text()

text = replace_once(
    text,
    '        Binding("r", "refresh", "Refresh"),',
    lines(
        '        Binding("r", "refresh", "Refresh"),',
        '        Binding("u", "toggle_timezone", "UTC / Local"),',
    ),
    "timezone binding",
)

text = replace_once(
    text,
    '    DexScreen { layout: vertical; background: $surface; padding: 1 2; }',
    '    DexScreen { layout: vertical; width: 100%; height: 100%; background: $background; opacity: 100%; padding: 1 2; }',
    "opaque dex screen",
)

text = replace_once(
    text,
    lines(
        '        with Horizontal(id="book-row"):',
        '            with Vertical(classes="book-pane"):',
        '                yield Label("ASK · SELL", classes="dex-section")',
        '                yield DataTable(id="dex-asks")',
        '            with Vertical(classes="book-pane"):',
        '                yield Label("BID · BUY", classes="dex-section")',
        '                yield DataTable(id="dex-bids")',
    ),
    lines(
        '        with Horizontal(id="book-row"):',
        '            with Vertical(classes="book-pane"):',
        '                yield Label("BID · BUY", classes="dex-section")',
        '                yield DataTable(id="dex-bids")',
        '            with Vertical(classes="book-pane"):',
        '                yield Label("ASK · SELL", classes="dex-section")',
        '                yield DataTable(id="dex-asks")',
    ),
    "book pane order",
)

text = replace_once(
    text,
    lines(
        '        asks = self.query_one("#dex-asks", DataTable)',
        '        asks.add_columns("Amount", "Price")',
        '        asks.cursor_type = "row"',
        '        bids = self.query_one("#dex-bids", DataTable)',
        '        bids.add_columns("Price", "Amount")',
        '        bids.cursor_type = "row"',
        '        trades = self.query_one("#dex-trades", DataTable)',
        '        trades.add_columns("Price", "Amount", "Time")',
        '        trades.cursor_type = "row"',
        '        offers = self.query_one("#dex-offers", DataTable)',
        '        offers.add_columns("Side", "Amount", "Price", "Total", "Offer ID")',
        '        offers.cursor_type = "row"',
        '        fills = self.query_one("#dex-fills", DataTable)',
        '        fills.add_columns("Time", "Side", "Amount", "Price", "Total", "Fills", "Offer")',
        '        fills.cursor_type = "row"',
    ),
    lines(
        '        bids = self.query_one("#dex-bids", DataTable)',
        '        bids.add_columns("Amount", "Price")',
        '        bids.cursor_type = "row"',
        '        asks = self.query_one("#dex-asks", DataTable)',
        '        asks.add_columns("Price", "Amount")',
        '        asks.cursor_type = "row"',
        '        trades = self.query_one("#dex-trades", DataTable)',
        '        trades.add_columns("Price", "Amount", self._time_column())',
        '        trades.cursor_type = "row"',
        '        offers = self.query_one("#dex-offers", DataTable)',
        '        offers.add_columns("Side", "Amount", "Price", "Total", "Offer ID")',
        '        offers.cursor_type = "row"',
        '        fills = self.query_one("#dex-fills", DataTable)',
        '        fills.add_columns(self._time_column(), "Side", "Amount", "Price", "Total", "Fills", "Offer")',
        '        fills.cursor_type = "row"',
    ),
    "book and time columns",
)

text = replace_once(
    text,
    lines(
        '    def action_refresh(self) -> None:',
        '        self.refresh_market()',
        '',
        '    def action_favorite_market(self) -> None:',
    ),
    lines(
        '    def action_refresh(self) -> None:',
        '        self.refresh_market()',
        '',
        '    def action_toggle_timezone(self) -> None:',
        '        settings = getattr(self.runtime, "settings", None)',
        '        if settings is None:',
        '            self.set_status("Timezone preference is unavailable in this runtime.")',
        '            return',
        '        settings.use_local_time = not bool(getattr(settings, "use_local_time", True))',
        '        store = getattr(self.runtime, "settings_store", None)',
        '        if store is not None:',
        '            store.save(settings)',
        '        self._update_time_columns()',
        '        self.refresh_market()',
        '',
        '    def action_favorite_market(self) -> None:',
    ),
    "timezone action",
)

text = replace_once(
    text,
    lines(
        '        for row in orderbook.get("asks", []):',
        '            amount = _decimal(row.get("amount", "0"))',
        '            asks.add_row(',
        '                format_amount(amount),',
        '                _stellar_decimal_text(_book_price(row), style="red"),',
        '            )',
        '        for row in orderbook.get("bids", []):',
        '            amount = _bid_base_amount(row)',
        '            bids.add_row(',
        '                _stellar_decimal_text(_book_price(row), style="green"),',
        '                format_amount(amount),',
        '            )',
    ),
    lines(
        '        for row in orderbook.get("asks", []):',
        '            amount = _decimal(row.get("amount", "0"))',
        '            asks.add_row(',
        '                _stellar_decimal_text(_book_price(row), style="red"),',
        '                _stellar_decimal_text(amount),',
        '            )',
        '        for row in orderbook.get("bids", []):',
        '            amount = _bid_base_amount(row)',
        '            bids.add_row(',
        '                _stellar_decimal_text(amount),',
        '                _stellar_decimal_text(_book_price(row), style="green"),',
        '            )',
    ),
    "orderbook mirrored rows",
)

text = replace_once(
    text,
    lines(
        '    def _time(self, value: str | None) -> str:',
        '        settings = getattr(self.runtime, "settings", None)',
        '        return format_timestamp(value, local=bool(getattr(settings, "use_local_time", True)))',
        '',
        '    def set_status(self, message: str) -> None:',
    ),
    lines(
        '    def _time(self, value: str | None) -> str:',
        '        settings = getattr(self.runtime, "settings", None)',
        '        return format_timestamp(value, local=bool(getattr(settings, "use_local_time", True)))',
        '',
        '    def _time_column(self) -> str:',
        '        settings = getattr(self.runtime, "settings", None)',
        '        use_local = bool(getattr(settings, "use_local_time", True))',
        '        return "Time (local)" if use_local else "Time (UTC)"',
        '',
        '    def _update_time_columns(self) -> None:',
        '        label = self._time_column()',
        '        trades = self.query_one("#dex-trades", DataTable)',
        '        trade_columns = list(trades.columns.values())',
        '        if trade_columns:',
        '            trade_columns[-1].label = Text(label)',
        '            trades.refresh()',
        '        fills = self.query_one("#dex-fills", DataTable)',
        '        fill_columns = list(fills.columns.values())',
        '        if fill_columns:',
        '            fill_columns[0].label = Text(label)',
        '            fills.refresh()',
        '',
        '    def set_status(self, message: str) -> None:',
    ),
    "time column helpers",
)

path.write_text(text)

path = Path("reference/python/tests/test_tui_dex_market_ux.py")
text = path.read_text()
text = replace_once(
    text,
    lines(
        '            assert [str(column.label) for column in asks.columns.values()] == ["Amount", "Price"]',
        '            assert [str(column.label) for column in bids.columns.values()] == ["Price", "Amount"]',
        '            assert [str(column.label) for column in trades.columns.values()] == ["Price", "Amount", "Time"]',
    ),
    lines(
        '            assert [str(column.label) for column in bids.columns.values()] == ["Amount", "Price"]',
        '            assert [str(column.label) for column in asks.columns.values()] == ["Price", "Amount"]',
        '            assert [str(column.label) for column in trades.columns.values()] == ["Price", "Amount", "Time (UTC)"]',
    ),
    "market ux columns",
)
text = replace_once(
    text,
    lines(
        '            assert _plain_row(asks, 0) == ["20", "0.5100000"]',
        '            assert _plain_row(bids, 0) == ["0.4100000", "10"]',
    ),
    lines(
        '            assert _plain_row(bids, 0) == ["10.0000000", "0.4100000"]',
        '            assert _plain_row(asks, 0) == ["0.5100000", "20.0000000"]',
    ),
    "market ux book rows",
)
path.write_text(text)

path = Path("reference/python/tests/test_tui_dex.py")
text = path.read_text()
text = replace_once(
    text,
    lines(
        '            assert _plain_row(asks, 0) == ["100", "0.3300000"]',
        '            assert _plain_row(bids, 0) == ["0.3200000", "625"]',
    ),
    lines(
        '            assert _plain_row(bids, 0) == ["625.0000000", "0.3200000"]',
        '            assert _plain_row(asks, 0) == ["0.3300000", "100.0000000"]',
    ),
    "dex projection book rows",
)
path.write_text(text)

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / "reference" / "python"


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"pattern not found in {path}: {old[:160]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


# StellarAdapter: account-offer pages need the same cursor/order controls already
# used by operations and account trades.
adapter = PY / "fresnica" / "stellar_adapter.py"
replace_once(
    adapter,
    '''    def get_offers(self, address: str, limit: int = 20) -> dict:\n        try:\n            return (\n                self.server.offers()\n                .for_account(address)\n                .order(desc=True)\n                .limit(limit)\n                .call()\n            )\n        except SdkError as exc:\n            raise NetworkError(\n                f"Unable to load offers for {address}",\n                details=_sdk_error_details(exc),\n            ) from exc\n''',
    '''    def get_offers(\n        self,\n        address: str,\n        limit: int = 20,\n        cursor: str | None = None,\n        desc: bool = True,\n    ) -> dict:\n        try:\n            builder = (\n                self.server.offers()\n                .for_account(address)\n                .order(desc=desc)\n                .limit(limit)\n            )\n            if cursor is not None:\n                builder = builder.cursor(cursor)\n            return builder.call()\n        except SdkError as exc:\n            raise NetworkError(\n                f"Unable to load offers for {address}",\n                details=_sdk_error_details(exc),\n            ) from exc\n''',
)

# DexService: offers are a full snapshot, while incremental account-trade sync
# stays bounded but explicitly reports whether it reached Horizon's current head.
dex_service = PY / "fresnica" / "dex_service.py"
replace_once(
    dex_service,
    '"""SDEX market and account data built on Horizon through the Stellar SDK."""\n\nfrom .models import Asset\n',
    '"""SDEX market and account data built on Horizon through the Stellar SDK."""\n\nfrom dataclasses import dataclass\n\nfrom .models import Asset\n',
)
replace_once(
    dex_service,
    '''ACCOUNT_TRADE_PAGE_LIMIT = 200\nACCOUNT_TRADE_MAX_INCREMENTAL_PAGES = 5\n\n\nclass DexService:\n''',
    '''OFFER_PAGE_LIMIT = 200\nACCOUNT_TRADE_PAGE_LIMIT = 200\nACCOUNT_TRADE_MAX_INCREMENTAL_PAGES = 5\n\n\n@dataclass(frozen=True)\nclass AccountTradeSegmentSnapshot:\n    segments: list\n    caught_up: bool\n\n\nclass DexService:\n''',
)
replace_once(
    dex_service,
    '''        self.adapter = adapter\n        self.datastore = datastore\n        self.network_name = network_name\n''',
    '''        self.adapter = adapter\n        self.datastore = datastore\n        self.network_name = network_name\n        self._account_trade_caught_up: dict[str, bool] = {}\n''',
)
replace_once(
    dex_service,
    '''    def get_offers(self, wallet, limit: int = 20, refresh: bool = True) -> list[dict]:\n        address = wallet.address()\n        if refresh:\n            response = self.adapter.get_offers(address, limit=limit)\n            self.datastore.save_offers(self.network_name, address, response)\n            return _records(response)\n        return self.datastore.get_offers(self.network_name, address, limit=limit)\n''',
    '''    def get_offers(self, wallet, limit: int = 20, refresh: bool = True) -> list[dict]:\n        address = wallet.address()\n        if refresh:\n            self._sync_offers_snapshot(address)\n        return self.datastore.get_offers(self.network_name, address, limit=limit)\n\n    def _sync_offers_snapshot(self, address: str) -> int:\n        records: list[dict] = []\n        cursor: str | None = None\n        while True:\n            response = self.adapter.get_offers(\n                address,\n                limit=OFFER_PAGE_LIMIT,\n                cursor=cursor,\n                desc=True,\n            )\n            page = _records(response)\n            records.extend(page)\n            if len(page) < OFFER_PAGE_LIMIT:\n                break\n            next_cursor = _paging_token(page[-1])\n            if next_cursor is None or next_cursor == cursor:\n                raise ValueError("Offer pagination stalled before a complete snapshot")\n            cursor = next_cursor\n        self.datastore.save_offers(self.network_name, address, records)\n        return len(records)\n''',
)
start = '''    def get_account_trade_segments(\n        self,\n        wallet,\n        limit: int = 1000,\n        refresh: bool = True,\n    ):\n        """Sync wallet trades incrementally, then aggregate consecutive offer fills."""\n        address = wallet.address()\n        cache_key = account_trade_cache_key(address)\n        if refresh:\n            latest = self.datastore.get_trades(\n                self.network_name,\n                cache_key,\n                limit=1,\n            )\n            cursor = _paging_token(latest[0]) if latest else None\n            if cursor:\n                self._sync_account_trade_increment(address, cache_key, cursor)\n            else:\n                response = self.adapter.get_account_trades(\n                    address,\n                    limit=min(ACCOUNT_TRADE_PAGE_LIMIT, max(limit, 1)),\n                    desc=True,\n                )\n                self.datastore.save_trades(\n                    self.network_name,\n                    cache_key,\n                    response,\n                )\n\n        raw = self.datastore.get_trades(\n            self.network_name,\n            cache_key,\n            limit=limit,\n        )\n        trades = [account_trade_from_horizon(item, address) for item in raw]\n        return compress_account_trades(trades, address)\n\n    def _sync_account_trade_increment(\n        self,\n        address: str,\n        cache_key: str,\n        cursor: str,\n    ) -> None:\n        next_cursor = cursor\n        for _ in range(ACCOUNT_TRADE_MAX_INCREMENTAL_PAGES):\n            response = self.adapter.get_account_trades(\n                address,\n                limit=ACCOUNT_TRADE_PAGE_LIMIT,\n                cursor=next_cursor,\n                desc=False,\n            )\n            records = _records(response)\n            if not records:\n                return\n            self.datastore.save_trades(self.network_name, cache_key, records)\n            last_cursor = _paging_token(records[-1])\n            if last_cursor is None or last_cursor == next_cursor:\n                return\n            next_cursor = last_cursor\n            if len(records) < ACCOUNT_TRADE_PAGE_LIMIT:\n                return\n'''
replacement = '''    def get_account_trade_segments(\n        self,\n        wallet,\n        limit: int = 1000,\n        refresh: bool = True,\n    ):\n        return self.get_account_trade_segment_snapshot(\n            wallet,\n            limit=limit,\n            refresh=refresh,\n        ).segments\n\n    def get_account_trade_segment_snapshot(\n        self,\n        wallet,\n        limit: int = 1000,\n        refresh: bool = True,\n    ) -> AccountTradeSegmentSnapshot:\n        """Return cached fill segments plus whether bounded sync reached Horizon head."""\n        address = wallet.address()\n        cache_key = account_trade_cache_key(address)\n        caught_up = self._account_trade_caught_up.get(address, False)\n        if refresh:\n            latest = self.datastore.get_trades(\n                self.network_name,\n                cache_key,\n                limit=1,\n            )\n            cursor = _paging_token(latest[0]) if latest else None\n            if cursor:\n                caught_up = self._sync_account_trade_increment(address, cache_key, cursor)\n            else:\n                response = self.adapter.get_account_trades(\n                    address,\n                    limit=min(ACCOUNT_TRADE_PAGE_LIMIT, max(limit, 1)),\n                    desc=True,\n                )\n                self.datastore.save_trades(\n                    self.network_name,\n                    cache_key,\n                    response,\n                )\n                caught_up = True\n            self._account_trade_caught_up[address] = caught_up\n\n        raw = self.datastore.get_trades(\n            self.network_name,\n            cache_key,\n            limit=limit,\n        )\n        trades = [account_trade_from_horizon(item, address) for item in raw]\n        return AccountTradeSegmentSnapshot(\n            segments=compress_account_trades(trades, address),\n            caught_up=caught_up,\n        )\n\n    def _sync_account_trade_increment(\n        self,\n        address: str,\n        cache_key: str,\n        cursor: str,\n    ) -> bool:\n        next_cursor = cursor\n        for _ in range(ACCOUNT_TRADE_MAX_INCREMENTAL_PAGES):\n            response = self.adapter.get_account_trades(\n                address,\n                limit=ACCOUNT_TRADE_PAGE_LIMIT,\n                cursor=next_cursor,\n                desc=False,\n            )\n            records = _records(response)\n            if not records:\n                return True\n            self.datastore.save_trades(self.network_name, cache_key, records)\n            last_cursor = _paging_token(records[-1])\n            if last_cursor is None or last_cursor == next_cursor:\n                return False\n            next_cursor = last_cursor\n            if len(records) < ACCOUNT_TRADE_PAGE_LIMIT:\n                return True\n        return False\n'''
replace_once(dex_service, start, replacement)

# TUI: surface a partial bounded fill sync instead of silently presenting it as current.
dex_tui = PY / "fresnica" / "tui" / "dex.py"
replace_once(
    dex_tui,
    '        self._counts = (0, 0, 0, 0, 0)\n\n    def compose',
    '        self._counts = (0, 0, 0, 0, 0)\n        self._fills_caught_up = True\n\n    def compose',
)
replace_once(
    dex_tui,
    '''        self._counts = (0, 0, 0, 0, 0)\n        self._clear_market_tables()\n''',
    '''        self._counts = (0, 0, 0, 0, 0)\n        self._fills_caught_up = True\n        self._clear_market_tables()\n''',
)
replace_once(
    dex_tui,
    '''            segments = services.dex_service.get_account_trade_segments(\n                session.wallet,\n                limit=1000,\n                refresh=True,\n            )\n            fills = [\n''',
    '''            snapshot_getter = getattr(\n                services.dex_service,\n                "get_account_trade_segment_snapshot",\n                None,\n            )\n            if snapshot_getter is not None:\n                fill_snapshot = snapshot_getter(\n                    session.wallet,\n                    limit=1000,\n                    refresh=True,\n                )\n                segments = fill_snapshot.segments\n                fills_caught_up = bool(fill_snapshot.caught_up)\n            else:\n                segments = services.dex_service.get_account_trade_segments(\n                    session.wallet,\n                    limit=1000,\n                    refresh=True,\n                )\n                fills_caught_up = True\n            fills = [\n''',
)
replace_once(
    dex_tui,
    '''                orderbook,\n                offer_rows,\n                fills,\n                recent_trades,\n                None,\n''',
    '''                orderbook,\n                offer_rows,\n                fills,\n                fills_caught_up,\n                recent_trades,\n                None,\n''',
)
replace_once(
    dex_tui,
    '''                {},\n                [],\n                [],\n                [],\n                exc,\n''',
    '''                {},\n                [],\n                [],\n                False,\n                [],\n                exc,\n''',
)
replace_once(
    dex_tui,
    '''        orderbook,\n        offer_rows,\n        fills,\n        recent_trades,\n        error,\n''',
    '''        orderbook,\n        offer_rows,\n        fills,\n        fills_caught_up,\n        recent_trades,\n        error,\n''',
)
replace_once(
    dex_tui,
    '''        self._visible_fills = list(fills)\n        self._render_fills()\n\n        self._counts = (\n''',
    '''        self._visible_fills = list(fills)\n        self._fills_caught_up = bool(fills_caught_up)\n        self._render_fills()\n\n        self._counts = (\n''',
)
replace_once(
    dex_tui,
    '''        self.set_status(\n            f"{asks} asks · {bids} bids · {trades} trades · "\n            f"{offers} open offers · {fills} fill segments · {suffix}"\n        )\n''',
    '''        fill_sync = "" if self._fills_caught_up else " · fill sync partial · R continue"\n        self.set_status(\n            f"{asks} asks · {bids} bids · {trades} trades · "\n            f"{offers} open offers · {fills} fill segments · {suffix}{fill_sync}"\n        )\n''',
)

# Service tests: paginate all offers into one authoritative snapshot and verify
# bounded fill sync exposes partial/caught-up transitions.
test_dex = PY / "tests" / "test_dex_service.py"
replace_once(
    test_dex,
    'from fresnica.dex_service import DexService, asset_pair_key, resolution_value\n',
    'import fresnica.dex_service as dex_service_module\nfrom fresnica.dex_service import DexService, asset_pair_key, resolution_value\n',
)
replace_once(
    test_dex,
    '''        self.account_trade_baseline = []\n        self.account_trade_increment = []\n''',
    '''        self.account_trade_baseline = []\n        self.account_trade_increment = []\n        self.account_trade_increment_pages = {}\n        self.offer_pages = {}\n''',
)
replace_once(
    test_dex,
    '''    def get_offers(self, address, limit=20):\n        self.calls.append(("offers", address, limit))\n        return self.offers\n''',
    '''    def get_offers(self, address, limit=20, cursor=None, desc=True):\n        self.calls.append(("offers", address, limit, cursor, desc))\n        if self.offer_pages:\n            return {"_embedded": {"records": list(self.offer_pages.get(cursor, []))}}\n        return self.offers\n''',
)
replace_once(
    test_dex,
    '''        records = self.account_trade_baseline if cursor is None else self.account_trade_increment\n        return {"_embedded": {"records": list(records)}}\n''',
    '''        if cursor is None:\n            records = self.account_trade_baseline\n        else:\n            records = self.account_trade_increment_pages.get(\n                cursor, self.account_trade_increment\n            )\n        return {"_embedded": {"records": list(records)}}\n''',
)
replace_once(
    test_dex,
    '''    assert service.get_offers(wallet, limit=5) == adapter.offers["_embedded"]["records"]\n''',
    '''    assert service.get_offers(wallet, limit=5) == adapter.offers["_embedded"]["records"]\n    assert adapter.calls[-1] == ("offers", wallet.address(), 200, None, True)\n''',
)
append = '''\n\ndef test_account_offers_refresh_pages_complete_snapshot_and_removes_stale_cache(monkeypatch):\n    monkeypatch.setattr(dex_service_module, "OFFER_PAGE_LIMIT", 2)\n    adapter = FakeAdapter()\n    store = MemoryDataStore()\n    service = DexService(adapter, store, "mainnet")\n    wallet = Wallet.from_address(Keypair.random().public_key)\n    adapter.offer_pages = {\n        None: [\n            {"id": "4", "paging_token": "4"},\n            {"id": "3", "paging_token": "3"},\n        ],\n        "3": [\n            {"id": "2", "paging_token": "2"},\n            {"id": "1", "paging_token": "1"},\n        ],\n        "1": [],\n    }\n\n    assert [item["id"] for item in service.get_offers(wallet, limit=10)] == [\n        "4", "3", "2", "1"\n    ]\n    assert [call[3] for call in adapter.calls if call[0] == "offers"] == [None, "3", "1"]\n\n    adapter.offer_pages = {None: [{"id": "5", "paging_token": "5"}]}\n    assert [item["id"] for item in service.get_offers(wallet, limit=10)] == ["5"]\n    assert [item["id"] for item in service.get_offers(wallet, limit=10, refresh=False)] == ["5"]\n\n\ndef test_account_trade_snapshot_reports_bounded_incremental_sync_until_caught_up(monkeypatch):\n    monkeypatch.setattr(dex_service_module, "ACCOUNT_TRADE_PAGE_LIMIT", 2)\n    monkeypatch.setattr(dex_service_module, "ACCOUNT_TRADE_MAX_INCREMENTAL_PAGES", 2)\n    adapter = FakeAdapter()\n    store = MemoryDataStore()\n    service = DexService(adapter, store, "mainnet")\n    address = Keypair.random().public_key\n    wallet = Wallet.from_address(address)\n    issuer = Keypair.random().public_key\n    adapter.account_trade_baseline = [\n        _account_trade(address, issuer, "1-0", "2026-08-23T00:01:00Z"),\n    ]\n    baseline = service.get_account_trade_segment_snapshot(wallet)\n    assert baseline.caught_up is True\n\n    adapter.account_trade_increment_pages = {\n        "1-0": [\n            _account_trade(address, issuer, "2-0", "2026-08-23T00:02:00Z"),\n            _account_trade(address, issuer, "3-0", "2026-08-23T00:03:00Z"),\n        ],\n        "3-0": [\n            _account_trade(address, issuer, "4-0", "2026-08-23T00:04:00Z"),\n            _account_trade(address, issuer, "5-0", "2026-08-23T00:05:00Z"),\n        ],\n        "5-0": [\n            _account_trade(address, issuer, "6-0", "2026-08-23T00:06:00Z"),\n        ],\n    }\n    partial = service.get_account_trade_segment_snapshot(wallet)\n    assert partial.caught_up is False\n    assert partial.segments[0].trade_count == 5\n\n    caught_up = service.get_account_trade_segment_snapshot(wallet)\n    assert caught_up.caught_up is True\n    assert caught_up.segments[0].trade_count == 6\n    cached = service.get_account_trade_segment_snapshot(wallet, refresh=False)\n    assert cached.caught_up is True\n\n\ndef test_account_trade_increment_stall_is_never_reported_as_caught_up(monkeypatch):\n    monkeypatch.setattr(dex_service_module, "ACCOUNT_TRADE_PAGE_LIMIT", 2)\n    adapter = FakeAdapter()\n    store = MemoryDataStore()\n    service = DexService(adapter, store, "mainnet")\n    address = Keypair.random().public_key\n    wallet = Wallet.from_address(address)\n    issuer = Keypair.random().public_key\n    adapter.account_trade_baseline = [\n        _account_trade(address, issuer, "1-0", "2026-08-23T00:01:00Z"),\n    ]\n    service.get_account_trade_segment_snapshot(wallet)\n    adapter.account_trade_increment_pages = {\n        "1-0": [\n            _account_trade(address, issuer, "2-0", "2026-08-23T00:02:00Z"),\n            _account_trade(address, issuer, "1-0", "2026-08-23T00:01:00Z"),\n        ]\n    }\n    assert service.get_account_trade_segment_snapshot(wallet).caught_up is False\n'''
text = test_dex.read_text(encoding="utf-8")
if "test_account_offers_refresh_pages_complete_snapshot" in text:
    raise SystemExit("dex service tests already patched")
test_dex.write_text(text + append, encoding="utf-8")

# Adapter regression: cursor/order are applied to the official SDK builder.
test_adapter = PY / "tests" / "test_stellar_adapter.py"
append = '''\n\nclass OffersBuilder:\n    def __init__(self):\n        self.steps = []\n\n    def for_account(self, address):\n        self.steps.append(("account", address))\n        return self\n\n    def order(self, desc=True):\n        self.steps.append(("order", desc))\n        return self\n\n    def limit(self, limit):\n        self.steps.append(("limit", limit))\n        return self\n\n    def cursor(self, cursor):\n        self.steps.append(("cursor", cursor))\n        return self\n\n    def call(self):\n        return {"_embedded": {"records": []}}\n\n\nclass OffersServer:\n    def __init__(self):\n        self.builder = OffersBuilder()\n\n    def offers(self):\n        return self.builder\n\n\ndef test_adapter_account_offers_support_cursor_and_order():\n    adapter = StellarAdapter(TESTNET)\n    server = OffersServer()\n    adapter.server = server\n\n    assert adapter.get_offers("GACCOUNT", limit=200, cursor="123", desc=False) == {\n        "_embedded": {"records": []}\n    }\n    assert server.builder.steps == [\n        ("account", "GACCOUNT"),\n        ("order", False),\n        ("limit", 200),\n        ("cursor", "123"),\n    ]\n'''
text = test_adapter.read_text(encoding="utf-8")
if "test_adapter_account_offers_support_cursor_and_order" in text:
    raise SystemExit("adapter test already patched")
test_adapter.write_text(text + append, encoding="utf-8")

# TUI status formatter is deliberately small enough for a direct unit regression.
test_tui = PY / "tests" / "test_tui_dex_sync_status.py"
test_tui.write_text('''from stellar_sdk import Keypair\n\nfrom fresnica.models import Asset, MarketPair\nfrom fresnica.tui.dex import DexScreen\n\n\ndef test_dex_status_marks_partial_fill_sync_until_next_refresh():\n    pair = MarketPair(Asset("USD", Keypair.random().public_key), Asset("XLM"))\n    screen = DexScreen(object(), pair, lambda *args: None)\n    screen._counts = (20, 20, 30, 4, 10)\n    screen._fills_caught_up = False\n    messages = []\n    screen.set_status = messages.append\n\n    screen._set_market_status("snapshot loaded")\n\n    assert "10 fill segments" in messages[-1]\n    assert "fill sync partial" in messages[-1]\n    assert "R continue" in messages[-1]\n''', encoding="utf-8")

print("SDEX account sync patch applied")

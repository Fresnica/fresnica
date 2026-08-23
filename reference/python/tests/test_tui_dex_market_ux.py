import asyncio
from decimal import Decimal
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import Button, DataTable, Static

from fresnica.asset_catalog import AssetCatalogEntry
from fresnica.manager import WalletManager
from fresnica.market_preferences import MarketPreferencesStore
from fresnica.models import AccountTradeSegment, Asset, BalanceView, MarketPair, PriceRatio
from fresnica.settings import SettingsStore, UserSettings
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.dex import DexScreen, MarketPairDialog, _orderbook_grid


class BalanceService:
    def __init__(self, asset):
        self.asset = asset
        self.adapter = SimpleNamespace(account_exists=lambda address: True)

    def has_cached_account(self, wallet):
        return True

    def get_cached_portfolio_views(self, wallet):
        return self.get_portfolio_views(wallet)

    def get_portfolio_views(self, wallet):
        return (
            [
                BalanceView(
                    asset=Asset("XLM"),
                    balance=Decimal("50"),
                    selling_liabilities=Decimal("0"),
                    buying_liabilities=Decimal("0"),
                    available=Decimal("48"),
                ),
                BalanceView(
                    asset=self.asset,
                    balance=Decimal("100"),
                    selling_liabilities=Decimal("0"),
                    buying_liabilities=Decimal("0"),
                    available=Decimal("100"),
                ),
            ],
            [],
        )


class HistoryService:
    def get_activity_views(self, wallet, limit=20, refresh=True):
        return []

    get_views = get_activity_views


class Catalog:
    def __init__(self, entries):
        self.entries = entries

    def cached(self, network):
        return self.entries

    def recommended(self, network, limit=30, refresh=True):
        return self.entries


class DexService:
    def __init__(self, pair):
        self.pair = pair
        self.calls = []

    def get_orderbook(self, base, counter):
        self.calls.append((base, counter))
        return {
            "asks": [
                {"price": "0.5", "price_r": {"n": 1, "d": 2}, "amount": "10"}
            ],
            "bids": [
                {"price": "0.4", "price_r": {"n": 2, "d": 5}, "amount": "4"}
            ],
        }

    def get_trades(self, base, counter, limit=30, refresh=True):
        trade = {
            "id": "trade-100",
            "paging_token": "100-0",
            "ledger_close_time": "2026-08-23T06:00:00Z",
            "base_amount": "2",
            "counter_amount": "1",
            "price": {"n": 1, "d": 2},
            "base_is_seller": True,
        }
        # Duplicate REST records must not create duplicate rows.
        return [trade, dict(trade)]

    def get_open_offers(self, wallet, limit=200, refresh=True):
        return []

    def get_account_trade_segments(self, wallet, limit=1000, refresh=True):
        return [
            AccountTradeSegment(
                segment_key="immediate",
                pair=self.pair,
                side="sell",
                base_amount=Decimal("3"),
                counter_amount=Decimal("1"),
                price_r=PriceRatio(1, 3),
                user_offer_id="4885936293211082753",
                trade_count=1,
                first_time="2026-08-23T05:59:00Z",
                last_time="2026-08-23T05:59:00Z",
                first_trade_id="fill-1",
                last_trade_id="fill-1",
            )
        ]


class RealtimeAdapter:
    def stream_orderbook(self, base, counter):
        yield {
            "asks": [
                {"price": "0.51", "price_r": {"n": 51, "d": 100}, "amount": "20"}
            ],
            "bids": [
                {"price": "0.41", "price_r": {"n": 41, "d": 100}, "amount": "4.1"}
            ],
        }

    def stream_trades(self, base, counter, cursor=None):
        assert cursor == "100-0"
        # Horizon may replay the cursor edge. It must replace, not duplicate,
        # the REST row before a genuinely new trade arrives.
        yield {
            "id": "trade-100",
            "paging_token": "100-0",
            "ledger_close_time": "2026-08-23T06:00:00Z",
            "base_amount": "2",
            "counter_amount": "1",
            "price": {"n": 1, "d": 2},
            "base_is_seller": True,
        }
        yield {
            "id": "trade-101",
            "paging_token": "101-0",
            "ledger_close_time": "2026-08-23T06:01:00Z",
            "base_amount": "4",
            "counter_amount": "2.04",
            "price": {"n": 51, "d": 100},
            "base_is_seller": False,
        }


class Runtime:
    def __init__(self, tmp_path):
        self.network = "mainnet"
        self.settings_store = SettingsStore(tmp_path / "settings.json")
        self.settings = UserSettings(use_local_time=False)
        self.settings_store.save(self.settings)
        self.market_preferences = MarketPreferencesStore(tmp_path / "markets.json")
        self.wallet_storage = MemoryWalletStorage()
        self.wallet_manager = WalletManager(self.wallet_storage)
        self.keypair = Keypair.random()
        self.wallet_manager.import_secret(
            "main",
            self.keypair.secret,
            "pw",
            network="mainnet",
            make_default=True,
        )
        self.issuer = Keypair.random().public_key
        self.usdc_issuer = Keypair.random().public_key
        self.asset = Asset("XRP", self.issuer)
        self.usdc = Asset("USDC", self.usdc_issuer)
        self.pair = MarketPair(self.asset, Asset("XLM"))
        self.asset_catalog = Catalog(
            [
                AssetCatalogEntry(Asset("XLM"), source="native"),
                AssetCatalogEntry(self.usdc, domain="circle.com", name="USD Coin"),
                AssetCatalogEntry(self.asset, domain="example.org", name="XRP"),
            ]
        )
        self.balance_service = BalanceService(self.asset)
        self.history_service = HistoryService()
        self.dex_service = DexService(self.pair)
        self.adapter = RealtimeAdapter()
        self.services = SimpleNamespace(
            balance_service=self.balance_service,
            history_service=self.history_service,
            dex_service=self.dex_service,
            adapter=self.adapter,
            pending_transaction_service=None,
            testnet_service=None,
        )

    def services_for(self, network=None):
        return self.services


async def _settle(pilot, cycles=8):
    for _ in range(cycles):
        await pilot.pause(0.04)


def _plain_row(table, row):
    return [value.plain if hasattr(value, "plain") else str(value) for value in table.get_row_at(row)]


def test_market_entry_favorites_realtime_book_trades_immediate_fill_and_swap(tmp_path):
    async def scenario():
        runtime = Runtime(tmp_path)
        app = FresnicaApp(runtime)
        async with app.run_test(size=(150, 58)) as pilot:
            await _settle(pilot)
            await pilot.press("d")
            await _settle(pilot, 8)
            assert isinstance(app.screen, MarketPairDialog)
            markets = app.screen.query_one("#market-list", DataTable)
            assert app.screen.focused is markets
            assert app.screen.query_one("#favorites", Button).label.plain == "★ Favorites [F]"

            # Popular follows Fex's held-asset ordering: XRP/XLM has two held
            # legs and therefore ranks ahead of default XLM/USDC.
            assert markets.row_count == 3
            assert _plain_row(markets, 0)[1:] == ["XRP/XLM", "example.org", "Stellar native"]
            assert "Recent: —" in str(app.screen.query_one("#market-recent", Static).render())

            await pilot.press("space")
            await _settle(pilot, 2)
            preferences = runtime.market_preferences.get(
                "mainnet", runtime.keypair.public_key
            )
            assert preferences.favorites == (runtime.pair,)
            assert _plain_row(markets, 0)[0] == "★"

            await pilot.press("f")
            await _settle(pilot, 2)
            assert markets.row_count == 1
            assert _plain_row(markets, 0)[1] == "XRP/XLM"

            await pilot.press("enter")
            await _settle(pilot, 14)
            assert isinstance(app.screen, DexScreen)
            assert "★ Stellar DEX" in str(app.screen.query_one("#dex-title", Static).render())

            asks = app.screen.query_one("#dex-asks", Static)
            bids = app.screen.query_one("#dex-bids", Static)
            trades = app.screen.query_one("#dex-trades", DataTable)
            fills = app.screen.query_one("#dex-fills", DataTable)

            bid_grid = _orderbook_grid(app.screen._orderbook["bids"], "bid")
            ask_grid = _orderbook_grid(app.screen._orderbook["asks"], "ask")
            assert [column.justify for column in bid_grid.columns] == ["right", "right"]
            assert [column.justify for column in ask_grid.columns] == ["left", "left"]
            assert [str(column.label) for column in trades.columns.values()] == ["Price", "Amount", "Time (UTC)"]

            # SSE replaces the REST book. Bid raw amount is quote amount:
            # 4.1 XLM / 0.41 XLM/XRP = 10 XRP BASE amount. Header is cell 0.
            assert bid_grid.columns[0]._cells[1].plain == "10.0000000"
            assert bid_grid.columns[1]._cells[1].plain == "0.4100000"
            assert ask_grid.columns[0]._cells[1].plain == "0.5100000"
            assert ask_grid.columns[1]._cells[1].plain == "20.0000000"

            # REST duplicate + SSE cursor replay remain one trade-100 row;
            # trade-101 is the only new row. No mystery counter-amount Total.
            assert trades.row_count == 2
            assert _plain_row(trades, 0)[:2] == ["0.5100000", "4"]
            assert _plain_row(trades, 1)[:2] == ["0.5000000", "2"]
            assert fills.row_count == 1
            assert _plain_row(fills, 0)[3] == "0.3333333"
            assert _plain_row(fills, 0)[6] == "Immediate"

            assert "Buy [B]" == app.screen.query_one("#buy", Button).label.plain
            assert "Sell [S]" == app.screen.query_one("#sell", Button).label.plain
            assert "⇄ Swap [W]" == app.screen.query_one("#swap", Button).label.plain
            assert "★ Star [F]" == app.screen.query_one("#favorite", Button).label.plain
            status = str(app.screen.query_one("#dex-status", Static).render())
            assert "realtime order book + trades" in status

            network_calls = len(runtime.dex_service.calls)
            await pilot.press("u")
            await _settle(pilot, 3)
            assert runtime.settings_store.load().use_local_time is True
            assert len(runtime.dex_service.calls) == network_calls
            assert [str(column.label) for column in trades.columns.values()][-1] == "Time (local)"
            assert [str(column.label) for column in fills.columns.values()][0] == "Time (local)"

            preferences = runtime.market_preferences.get(
                "mainnet", runtime.keypair.public_key
            )
            assert preferences.recents[0] == runtime.pair

            await pilot.press("w")
            await _settle(pilot, 14)
            swapped = MarketPair(Asset("XLM"), runtime.asset)
            assert app.screen.pair == swapped
            assert "XLM/XRP" in str(app.screen.query_one("#dex-title", Static).render())
            assert runtime.dex_service.calls[-1] == (swapped.base, swapped.counter)
            preferences = runtime.market_preferences.get(
                "mainnet", runtime.keypair.public_key
            )
            assert preferences.recents[0] == swapped

    asyncio.run(scenario())

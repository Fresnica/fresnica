import asyncio
from decimal import Decimal
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import DataTable, Static

from fresnica.manager import WalletManager
from fresnica.market_preferences import MarketPreferencesStore
from fresnica.models import AccountTradeSegment, Asset, BalanceView, MarketPair, PriceRatio
from fresnica.settings import SettingsStore, UserSettings
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.dex import DexScreen, MarketPairDialog


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


class DexService:
    def __init__(self, pair):
        self.pair = pair

    def get_orderbook(self, base, counter):
        return {
            "asks": [
                {"price": "0.5", "price_r": {"n": 1, "d": 2}, "amount": "10"}
            ],
            "bids": [
                {"price": "0.4", "price_r": {"n": 2, "d": 5}, "amount": "4"}
            ],
        }

    def get_trades(self, base, counter, limit=30, refresh=True):
        return [
            {
                "id": "trade-100",
                "paging_token": "100-0",
                "ledger_close_time": "2026-08-23T06:00:00Z",
                "base_amount": "2",
                "counter_amount": "1",
                "price": {"n": 1, "d": 2},
                "base_is_seller": True,
            }
        ]

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
        self.network = "testnet"
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
            network="testnet",
            make_default=True,
        )
        self.issuer = Keypair.random().public_key
        self.asset = Asset("XRP", self.issuer)
        self.pair = MarketPair(self.asset, Asset("XLM"))
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


def test_market_entry_starred_realtime_two_sided_book_and_immediate_fill(tmp_path):
    async def scenario():
        runtime = Runtime(tmp_path)
        app = FresnicaApp(runtime)
        async with app.run_test(size=(150, 56)) as pilot:
            await _settle(pilot)
            await pilot.press("d")
            await _settle(pilot, 4)
            assert isinstance(app.screen, MarketPairDialog)
            markets = app.screen.query_one("#market-list", DataTable)
            assert markets.row_count == 1
            assert markets.get_row_at(0)[1] == "XRP/XLM"
            assert markets.get_row_at(0)[4] == "Held asset"

            await pilot.press("f")
            await _settle(pilot, 2)
            preferences = runtime.market_preferences.get(
                "testnet", runtime.keypair.public_key
            )
            assert preferences.favorites == (runtime.pair,)
            assert markets.get_row_at(0)[0] == "★"

            await pilot.press("enter")
            await _settle(pilot, 12)
            assert isinstance(app.screen, DexScreen)
            assert "★ Stellar DEX" in str(app.screen.query_one("#dex-title", Static).render())

            asks = app.screen.query_one("#dex-asks", DataTable)
            bids = app.screen.query_one("#dex-bids", DataTable)
            trades = app.screen.query_one("#dex-trades", DataTable)
            fills = app.screen.query_one("#dex-fills", DataTable)

            # SSE snapshot replaces the initial REST book. Bid raw amount is quote
            # amount; 4.1 XLM / 0.41 XLM/XRP = 10 XRP displayed as BASE amount.
            assert asks.get_row_at(0) == ["0.51", "20", "10.2"]
            assert bids.get_row_at(0) == ["0.41", "10", "4.1"]
            assert trades.get_row_at(0)[1:] == ["BUY", "4", "0.51", "2.04"]
            assert fills.get_row_at(0)[3] == "0.3333333333"
            assert fills.get_row_at(0)[6] == "Immediate"
            status = str(app.screen.query_one("#dex-status", Static).render())
            assert "realtime order book + trades" in status

            preferences = runtime.market_preferences.get(
                "testnet", runtime.keypair.public_key
            )
            assert preferences.recents[0] == runtime.pair

    asyncio.run(scenario())

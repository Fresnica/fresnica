import asyncio
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import DataTable, Input, Static

from fresnica.manager import WalletManager, WalletState
from fresnica.models import (
    AccountTradeSegment,
    Asset,
    MarketPair,
    OpenOffer,
    PriceRatio,
    TransactionResult,
)
from fresnica.review import OfferReview
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.dex import DexScreen, MarketPairDialog, OfferFormDialog, OfferReviewDialog
from fresnica.tui.screens import NoticeDialog, UnlockDialog


PASSWORD = "test-password"


async def _settle(pilot, cycles=6):
    for _ in range(cycles):
        await pilot.pause(0.05)


class FakeBalanceService:
    def __init__(self):
        self.refreshes = 0
        self.adapter = SimpleNamespace(account_exists=lambda address: True)

    def has_cached_account(self, wallet):
        return True

    def get_cached_portfolio_views(self, wallet):
        return [], []

    def get_portfolio_views(self, wallet):
        self.refreshes += 1
        return [], []


class FakeHistoryService:
    def __init__(self):
        self.refreshes = 0

    def get_activity_views(self, wallet, limit=20, refresh=True):
        if refresh:
            self.refreshes += 1
        return []

    get_views = get_activity_views


class FakeDexService:
    def __init__(self, pair):
        self.pair = pair
        self.refreshes = 0
        self.buy_offer = OpenOffer(
            offer_id="42",
            selling=pair.counter,
            buying=pair.base,
            selling_amount=32.5,
            price_r=PriceRatio(40, 13),
        )
        self.fill = AccountTradeSegment(
            segment_key="raw",
            pair=MarketPair(pair.counter, pair.base),
            side="sell",
            base_amount=32.5,
            counter_amount=100,
            price_r=PriceRatio(40, 13),
            user_offer_id="42",
            trade_count=3,
            first_time="2026-08-23T00:00:00Z",
            last_time="2026-08-23T00:05:00Z",
            first_trade_id="t1",
            last_trade_id="t3",
        )

    def get_orderbook(self, base, counter):
        assert base == self.pair.base
        assert counter == self.pair.counter
        self.refreshes += 1
        return {
            "asks": [{"price": "0.33", "amount": "100"}],
            "bids": [{"price": "0.32", "amount": "200"}],
        }

    def get_open_offers(self, wallet, limit=200, refresh=True):
        return [self.buy_offer]

    def get_account_trade_segments(self, wallet, limit=1000, refresh=True):
        return [self.fill]


class FakeOfferService:
    def __init__(self):
        self.prepared = []
        self.signed = 0
        self.submitted = 0

    def prepare_create(self, wallet_name, wallet, intent, allow_trustline=False):
        self.prepared.append(("create", intent, allow_trustline))
        return SimpleNamespace(
            envelope=object(),
            review=OfferReview(
                wallet_name=wallet_name,
                source=wallet.address(),
                action="create",
                side=intent.side,
                base_asset=_identity(intent.pair.base),
                counter_asset=_identity(intent.pair.counter),
                amount=str(intent.amount),
                price=str(intent.price),
                total=str(intent.amount * intent.price),
                fee="0.00001",
                network="testnet",
            ),
        )

    def prepare_update(self, wallet_name, wallet, offer, intent):
        self.prepared.append(("update", offer, intent))
        return SimpleNamespace(
            envelope=object(),
            review=OfferReview(
                wallet_name=wallet_name,
                source=wallet.address(),
                action="update",
                side=intent.side,
                base_asset=_identity(intent.pair.base),
                counter_asset=_identity(intent.pair.counter),
                amount=str(intent.amount),
                price=str(intent.price),
                total=str(intent.amount * intent.price),
                fee="0.00001",
                network="testnet",
                offer_id=offer.offer_id,
            ),
        )

    def prepare_cancel(self, wallet_name, wallet, offer):
        self.prepared.append(("cancel", offer))
        return SimpleNamespace(
            envelope=object(),
            review=OfferReview(
                wallet_name=wallet_name,
                source=wallet.address(),
                action="cancel",
                side=None,
                base_asset=_identity(offer.selling),
                counter_asset=_identity(offer.buying),
                amount=None,
                price=None,
                total=None,
                fee="0.00001",
                network="testnet",
                offer_id=offer.offer_id,
            ),
        )

    def sign(self, wallet, prepared):
        self.signed += 1
        return prepared

    def submit(self, prepared):
        self.submitted += 1
        return TransactionResult(
            hash="dex-hash",
            ledger=123,
            successful=True,
        )


class FakeRuntime:
    def __init__(self, watch_only=False):
        self.network = "testnet"
        self.settings = SimpleNamespace(show_zero_balances=False)
        self.settings_store = SimpleNamespace(save=lambda settings: None)
        self.wallet_storage = MemoryWalletStorage()
        self.wallet_manager = WalletManager(self.wallet_storage)
        self.issuer = Keypair.random().public_key
        self.pair = MarketPair(Asset("XRP", self.issuer), Asset("XLM"))
        if watch_only:
            self.wallet_manager.add_watch(
                "main",
                Keypair.random().public_key,
                network="testnet",
                make_default=True,
            )
        else:
            self.keypair = Keypair.random()
            self.wallet_manager.import_secret(
                "main",
                self.keypair.secret,
                PASSWORD,
                network="testnet",
                make_default=True,
            )
        self.balance_service = FakeBalanceService()
        self.history_service = FakeHistoryService()
        self.dex_service = FakeDexService(self.pair)
        self.offer_service = FakeOfferService()
        self.services = SimpleNamespace(
            balance_service=self.balance_service,
            history_service=self.history_service,
            dex_service=self.dex_service,
            offer_service=self.offer_service,
            testnet_service=None,
        )

    def services_for(self, network=None):
        return self.services


def _identity(asset):
    return "XLM" if asset.is_native else f"{asset.code}:{asset.issuer}"


def _open_pair(app, runtime):
    dialog = app.screen
    assert isinstance(dialog, MarketPairDialog)
    dialog.query_one("#base", Input).value = f"XRP:{runtime.issuer}"
    dialog.query_one("#counter", Input).value = "XLM"


def test_dex_screen_projects_reverse_offer_and_fill_into_selected_pair():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(130, 50)) as pilot:
            await _settle(pilot)
            await pilot.press("d")
            _open_pair(app, runtime)
            await pilot.click("#open")
            await _settle(pilot, 10)

            assert isinstance(app.screen, DexScreen)
            offers = app.screen.query_one("#dex-offers", DataTable)
            fills = app.screen.query_one("#dex-fills", DataTable)
            book = app.screen.query_one("#dex-book", DataTable)
            assert book.row_count == 2
            assert offers.row_count == 1
            assert fills.row_count == 1
            assert offers.get_row_at(0) == ["BUY", "100", "0.325", "32.5", "42"]
            assert fills.get_row_at(0)[1:] == ["BUY", "100", "0.325", "32.5", "3", "42"]
            assert "1 open offers" in str(
                app.screen.query_one("#dex-status", Static).render()
            )

    asyncio.run(scenario())


def test_locked_wallet_resumes_buy_after_unlock_and_reuses_offer_pipeline():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(130, 50)) as pilot:
            await _settle(pilot)
            assert runtime.wallet_manager.state() is WalletState.LOCKED

            await pilot.press("d")
            _open_pair(app, runtime)
            await pilot.click("#open")
            await _settle(pilot, 8)
            assert isinstance(app.screen, DexScreen)

            await pilot.press("b")
            assert isinstance(app.screen, OfferFormDialog)
            app.screen.query_one("#amount", Input).value = "100"
            app.screen.query_one("#price", Input).value = "0.325"
            await pilot.click("#review")
            await _settle(pilot)

            assert isinstance(app.screen, UnlockDialog)
            app.screen.query_one("#unlock-password", Input).value = PASSWORD
            await pilot.click("#unlock")
            await _settle(pilot, 8)

            assert runtime.wallet_manager.state() is WalletState.UNLOCKED
            assert isinstance(app.screen, OfferReviewDialog)
            assert runtime.offer_service.prepared[0][0] == "create"
            intent = runtime.offer_service.prepared[0][1]
            assert intent.side == "buy"
            assert intent.amount == 100
            assert intent.price == 0.325

            await pilot.click("#confirm")
            await _settle(pilot, 10)
            assert isinstance(app.screen, DexScreen)
            assert runtime.offer_service.signed == 1
            assert runtime.offer_service.submitted == 1
            assert runtime.wallet_manager.state() is WalletState.UNLOCKED
            assert runtime.dex_service.refreshes >= 2
            assert runtime.balance_service.refreshes >= 2

    asyncio.run(scenario())


def test_watch_only_wallet_can_read_market_but_cannot_start_write():
    async def scenario():
        runtime = FakeRuntime(watch_only=True)
        app = FresnicaApp(runtime)
        async with app.run_test(size=(130, 50)) as pilot:
            await _settle(pilot)
            await pilot.press("d")
            _open_pair(app, runtime)
            await pilot.click("#open")
            await _settle(pilot, 8)
            assert isinstance(app.screen, DexScreen)

            await pilot.press("b")
            assert isinstance(app.screen, OfferFormDialog)
            app.screen.query_one("#amount", Input).value = "1"
            app.screen.query_one("#price", Input).value = "1"
            await pilot.click("#review")
            await _settle(pilot)

            assert isinstance(app.screen, NoticeDialog)
            assert runtime.offer_service.prepared == []

    asyncio.run(scenario())

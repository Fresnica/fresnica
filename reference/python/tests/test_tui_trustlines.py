import asyncio
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import DataTable, Input, Static

from fresnica.asset_catalog import AssetCatalogEntry
from fresnica.manager import WalletManager, WalletState
from fresnica.models import Asset
from fresnica.review import TrustlineReview
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.asset_picker import AssetPickerDialog
from fresnica.tui.review_dialog import ReviewPresentationDialog
from fresnica.tui.screens import NoticeDialog, UnlockDialog
from fresnica.trustline_policy import FRESNICA_TRUSTLINE_LIMIT_TEXT
from fresnica.tui.trustlines import TrustlineFormDialog, TrustlineScreen


PASSWORD = "test-password"


async def _settle(pilot, cycles=6):
    for _ in range(cycles):
        await pilot.pause(0.05)


class FakeBalanceService:
    def __init__(self, line):
        self.line = line
        self.account_reads = 0
        self.portfolio_reads = 0

    def get_account(self, wallet, refresh=True):
        self.account_reads += 1
        return {
            "balances": [
                {
                    "asset_type": "native",
                    "balance": "100",
                    "selling_liabilities": "0",
                    "buying_liabilities": "0",
                },
                self.line,
            ],
            "subentry_count": 1,
            "num_sponsoring": 0,
            "num_sponsored": 0,
        }

    def get_portfolio_views(self, wallet):
        self.portfolio_reads += 1
        return [], []

    def get_cached_portfolio_views(self, wallet):
        return [], []

    def has_cached_account(self, wallet):
        return True


class FakeHistoryService:
    def get_activity_views(self, wallet, limit=20, refresh=True):
        return []

    get_views = get_activity_views


class FakeTrustlineService:
    def __init__(self):
        self.calls = []
        self.signed = 0
        self.submitted = 0

    def prepare_add(self, wallet_name, wallet, asset, limit=None):
        self.calls.append(("add", asset, limit))
        return self._prepared(wallet_name, wallet, "add", asset, limit or "Stellar maximum")

    def prepare_limit(self, wallet_name, wallet, asset, limit):
        self.calls.append(("limit", asset, limit))
        return self._prepared(wallet_name, wallet, "limit", asset, limit)

    def prepare_remove(self, wallet_name, wallet, asset):
        self.calls.append(("remove", asset, None))
        return self._prepared(wallet_name, wallet, "remove", asset, None)

    def _prepared(self, wallet_name, wallet, action, asset, limit):
        return SimpleNamespace(
            review=TrustlineReview(
                wallet_name=wallet_name,
                source=wallet.address(),
                action=action,
                asset=asset,
                limit=limit,
                fee="0.00001",
                network="mainnet",
            ),
            envelope=object(),
        )

    def sign(self, wallet, prepared):
        self.signed += 1

    def submit(self, prepared):
        self.submitted += 1
        return SimpleNamespace(hash="trust-hash", ledger=77)


class FakeAssetCatalog:
    def __init__(self, entry):
        self.entry = entry

    def cached(self, network):
        return [AssetCatalogEntry(Asset("XLM"), source="native"), self.entry]

    def recommended(self, network, limit=30, refresh=True):
        return self.cached(network)


class FakeRuntime:
    def __init__(self, watch_only=False):
        self.network = "mainnet"
        self.settings = SimpleNamespace(show_zero_balances=False)
        self.settings_store = SimpleNamespace(save=lambda settings: None)
        self.wallet_manager = WalletManager(MemoryWalletStorage())
        self.issuer = Keypair.random().public_key
        self.asset = f"USD:{self.issuer}"
        self.recommended_issuer = Keypair.random().public_key
        self.recommended_asset = f"EUR:{self.recommended_issuer}"
        self.asset_catalog = FakeAssetCatalog(
            AssetCatalogEntry(
                Asset("EUR", self.recommended_issuer),
                domain="example.org",
                name="Example Euro",
            )
        )
        self.line = {
            "asset_type": "credit_alphanum4",
            "asset_code": "USD",
            "asset_issuer": self.issuer,
            "balance": "0",
            "limit": "100.0000000",
            "buying_liabilities": "0",
            "selling_liabilities": "0",
        }
        if watch_only:
            self.wallet_manager.add_watch(
                "main",
                Keypair.random().public_key,
                network="mainnet",
                make_default=True,
            )
        else:
            self.keypair = Keypair.random()
            self.wallet_manager.import_secret(
                "main",
                self.keypair.secret,
                PASSWORD,
                network="mainnet",
                make_default=True,
            )
        self.balance_service = FakeBalanceService(self.line)
        self.history_service = FakeHistoryService()
        self.trustline_service = FakeTrustlineService()
        self.services = SimpleNamespace(
            balance_service=self.balance_service,
            history_service=self.history_service,
            trustline_service=self.trustline_service,
            pending_transaction_service=None,
            testnet_service=None,
        )

    def services_for(self, network=None):
        return self.services


async def _enter_trustlines(app, pilot):
    await pilot.press("t")
    await _settle(pilot, 8)
    assert isinstance(app.screen, TrustlineScreen)


def test_trustline_screen_lists_full_asset_identity_and_limits():
    async def scenario():
        runtime = FakeRuntime(watch_only=True)
        app = FresnicaApp(runtime)
        async with app.run_test(size=(130, 45)) as pilot:
            await _settle(pilot)
            await _enter_trustlines(app, pilot)

            table = app.screen.query_one("#trustlines", DataTable)
            assert table.row_count == 1
            row = table.get_row_at(0)
            assert row[0] == runtime.asset
            assert row[1:] == ["0", "100", "0", "0"]

    asyncio.run(scenario())


def test_locked_wallet_add_uses_asset_picker_then_resumes_shared_pipeline():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(130, 45)) as pilot:
            await _settle(pilot)
            assert runtime.wallet_manager.state() is WalletState.LOCKED
            await _enter_trustlines(app, pilot)

            await pilot.press("a")
            await _settle(pilot, 4)
            assert isinstance(app.screen, AssetPickerDialog)
            picker = app.screen.query_one("#asset-picker-table", DataTable)
            assert picker.row_count == 1
            assert picker.get_row_at(0)[0] == "EUR"
            await pilot.press("enter")
            await _settle(pilot, 3)

            assert isinstance(app.screen, TrustlineFormDialog)
            assert str(app.screen.query_one("#asset-label", Static).render()) == runtime.recommended_asset
            assert app.screen.query_one("#limit", Input).value == FRESNICA_TRUSTLINE_LIMIT_TEXT
            await pilot.click("#review")
            await _settle(pilot)

            assert isinstance(app.screen, UnlockDialog)
            app.screen.query_one("#unlock-password", Input).value = PASSWORD
            await pilot.click("#unlock")
            await _settle(pilot, 8)

            assert runtime.wallet_manager.state() is WalletState.UNLOCKED
            assert runtime.trustline_service.calls == [
                ("add", runtime.recommended_asset, FRESNICA_TRUSTLINE_LIMIT_TEXT)
            ]
            assert isinstance(app.screen, ReviewPresentationDialog)
            text = str(app.screen.query_one("#review-text", Static).render())
            assert f"Add trustline for {runtime.recommended_asset}" in text
            assert f"Limit: {FRESNICA_TRUSTLINE_LIMIT_TEXT}" in text

            await pilot.click("#confirm")
            await _settle(pilot, 10)
            assert isinstance(app.screen, TrustlineScreen)
            assert runtime.trustline_service.signed == 1
            assert runtime.trustline_service.submitted == 1
            assert runtime.wallet_manager.state() is WalletState.UNLOCKED
            assert runtime.balance_service.account_reads >= 2

    asyncio.run(scenario())


def test_selected_trustline_limit_and_remove_keep_full_asset_identity():
    async def scenario():
        runtime = FakeRuntime()
        runtime.wallet_manager.unlock("main", PASSWORD)
        app = FresnicaApp(runtime)
        async with app.run_test(size=(130, 45)) as pilot:
            await _settle(pilot)
            await _enter_trustlines(app, pilot)

            await pilot.press("e")
            assert isinstance(app.screen, TrustlineFormDialog)
            assert str(app.screen.query_one("#asset-label", Static).render()) == runtime.asset
            app.screen.query_one("#limit", Input).value = "250"
            await pilot.click("#review")
            await _settle(pilot, 6)

            assert runtime.trustline_service.calls[-1] == ("limit", runtime.asset, "250")
            assert isinstance(app.screen, ReviewPresentationDialog)
            await pilot.click("#cancel")
            await _settle(pilot)
            assert isinstance(app.screen, TrustlineScreen)

            await pilot.press("x")
            await _settle(pilot, 6)
            assert runtime.trustline_service.calls[-1] == ("remove", runtime.asset, None)
            assert isinstance(app.screen, ReviewPresentationDialog)
            text = str(app.screen.query_one("#review-text", Static).render())
            assert f"Remove trustline for {runtime.asset}" in text
            assert "Limit: 0" not in text

    asyncio.run(scenario())


def test_watch_only_wallet_can_view_but_cannot_prepare_trustline_write():
    async def scenario():
        runtime = FakeRuntime(watch_only=True)
        app = FresnicaApp(runtime)
        async with app.run_test(size=(130, 45)) as pilot:
            await _settle(pilot)
            await _enter_trustlines(app, pilot)

            await pilot.press("x")
            await _settle(pilot)

            assert isinstance(app.screen, NoticeDialog)
            assert runtime.trustline_service.calls == []

    asyncio.run(scenario())

import asyncio
from decimal import Decimal
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import DataTable, Input, Static

from fresnica.balance_service import ISSUER_DOMAIN_CACHE_KEY
from fresnica.contacts import ContactStore
from fresnica.manager import WalletManager
from fresnica.models import ActivityView, Asset, BalanceView, OperationView
from fresnica.settings import SettingsStore, UserSettings
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.asset_details import AssetDetailsScreen, PrefilledSendDialog
from fresnica.tui.contact_book import AddContactDialog, ContactBookScreen
from fresnica.tui.history import ActivityDetailDialog, HistoryScreen


class UXBalanceService:
    def __init__(self, issuer):
        self.issuer = issuer

    def _portfolio(self):
        return (
            [
                BalanceView(
                    asset=Asset("XLM"),
                    balance=Decimal("25"),
                    selling_liabilities=Decimal("0"),
                    buying_liabilities=Decimal("0"),
                    available=Decimal("23"),
                    raw={"asset_type": "native"},
                ),
                BalanceView(
                    asset=Asset("USDC", self.issuer),
                    balance=Decimal("50"),
                    selling_liabilities=Decimal("5"),
                    buying_liabilities=Decimal("0"),
                    available=Decimal("45"),
                    raw={
                        "asset_type": "credit_alphanum4",
                        "asset_code": "USDC",
                        "asset_issuer": self.issuer,
                        "limit": "1000.0000000",
                        "is_authorized": True,
                        ISSUER_DOMAIN_CACHE_KEY: "anchor.example",
                    },
                ),
            ],
            [],
        )

    def has_cached_account(self, wallet):
        return True

    def get_cached_portfolio_views(self, wallet):
        return self._portfolio()

    def get_portfolio_views(self, wallet):
        return self._portfolio()


class UXHistoryService:
    def __init__(self, account, sender):
        payment = OperationView(
            operation_type="payment",
            created_at="2026-08-23T06:00:00Z",
            summary=f"Received 2 XLM from {sender[:6]}...{sender[-4:]}",
            raw={
                "paging_token": "200",
                "transaction_hash": "tx-normal",
                "type": "payment",
                "from": sender,
                "to": account,
            },
        )
        data = OperationView(
            operation_type="manage_data",
            created_at="2026-08-23T06:00:00Z",
            summary="Updated account data: profile",
            raw={
                "paging_token": "199",
                "transaction_hash": "tx-normal",
                "type": "manage_data",
                "source_account": account,
            },
        )
        dust_op = OperationView(
            operation_type="create_claimable_balance",
            created_at="2026-08-23T05:00:00Z",
            summary="Incoming claimable asset: 0.0000001 SPAM · review before claiming",
            raw={
                "paging_token": "198",
                "transaction_hash": "tx-dust",
                "type": "create_claimable_balance",
                "source_account": sender,
                "_fresnica_unsolicited_claimable": True,
            },
        )
        self.activities = [
            ActivityView(
                operation_type="transaction",
                created_at=payment.created_at,
                summary="2 actions · Received 2 XLM · Updated account data: profile",
                transaction_hash="tx-normal",
                operation_count=2,
                operations=[payment, data],
                raw=[payment.raw, data.raw],
            ),
            ActivityView(
                operation_type="create_claimable_balance",
                created_at=dust_op.created_at,
                summary=dust_op.summary,
                transaction_hash="tx-dust",
                operation_count=1,
                operations=[dust_op],
                raw=[dust_op.raw],
            ),
        ]
        self.refreshes = 0
        self.older = 0

    def get_activity_views(self, wallet, limit=20, refresh=True):
        if refresh:
            self.refreshes += 1
        return self.activities[:limit]

    def get_views(self, wallet, limit=20, refresh=True):
        return self.get_activity_views(wallet, limit=limit, refresh=refresh)

    def sync_recent(self, wallet):
        self.refreshes += 1
        return 0

    def load_older(self, wallet, limit=200):
        self.older += 1
        return 0

    def cached_operation_count(self, wallet):
        return 3


class UXRuntime:
    def __init__(self, tmp_path, *, theme=None):
        storage = MemoryWalletStorage()
        self.wallet_manager = WalletManager(storage)
        self.keypair = Keypair.random()
        self.sender = Keypair.random().public_key
        self.issuer = Keypair.random().public_key
        self.wallet_manager.import_secret(
            "alpha",
            self.keypair.secret,
            "pw",
            network="mainnet",
            make_default=True,
        )
        self.network = "mainnet"
        self.settings_store = SettingsStore(tmp_path / "settings.json")
        self.settings = UserSettings(theme=theme)
        self.settings_store.save(self.settings)
        self.contact_store = ContactStore(tmp_path / "contacts.json")
        self.balance_service = UXBalanceService(self.issuer)
        self.history_service = UXHistoryService(self.keypair.public_key, self.sender)

    def services_for(self, network=None):
        return SimpleNamespace(
            balance_service=self.balance_service,
            history_service=self.history_service,
            pending_transaction_service=None,
        )


async def _settle(pilot, rounds=8):
    for _ in range(rounds):
        await pilot.pause(0.03)


def test_theme_is_loaded_and_future_theme_changes_are_persisted(tmp_path):
    async def scenario():
        runtime = UXRuntime(tmp_path, theme="textual-light")
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot)
            assert app.theme == "textual-light"
            app.theme = "textual-dark"
            await _settle(pilot, 3)
            assert runtime.settings_store.load().theme == "textual-dark"

    asyncio.run(scenario())


def test_balance_row_send_prefills_asset_and_enter_opens_asset_details(tmp_path):
    async def scenario():
        runtime = UXRuntime(tmp_path)
        runtime.wallet_manager.unlock("alpha", "pw")
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot)
            balances = app.query_one("#balances", DataTable)
            assert balances.row_count == 2
            balances.move_cursor(row=1)
            app.set_focus(balances)

            await pilot.press("s")
            await _settle(pilot, 3)
            assert isinstance(app.screen, PrefilledSendDialog)
            assert app.screen.query_one("#asset", Input).value == f"USDC:{runtime.issuer}"

            await pilot.press("escape")
            await _settle(pilot, 3)
            balances = app.query_one("#balances", DataTable)
            balances.move_cursor(row=1)
            app.set_focus(balances)
            await pilot.press("enter")
            await _settle(pilot, 3)
            assert isinstance(app.screen, AssetDetailsScreen)
            assert "anchor.example" in str(app.screen.query_one("#asset-anchor", Static).render())
            assert "press A" in str(app.screen.query_one("#asset-anchor", Static).render())
            assert len(app.screen.query("#discover-anchor")) == 1

    asyncio.run(scenario())


def test_history_dust_timezone_details_and_contact_hook(tmp_path):
    async def scenario():
        runtime = UXRuntime(tmp_path)
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 46)) as pilot:
            await _settle(pilot)
            await pilot.press("h")
            await _settle(pilot)
            assert isinstance(app.screen, HistoryScreen)
            table = app.screen.query_one("#history-table", DataTable)
            assert table.row_count == 1
            assert "3 operations cached" in str(
                app.screen.query_one("#history-status", Static).render()
            )
            assert "local time" in str(app.screen.query_one("#history-status", Static).render())

            await pilot.press("d")
            await _settle(pilot, 4)
            assert table.row_count == 2
            assert runtime.settings_store.load().show_dust_activity is True

            await pilot.press("d")
            await _settle(pilot, 4)
            assert table.row_count == 1

            await pilot.press("u")
            await _settle(pilot, 4)
            assert runtime.settings_store.load().use_local_time is False
            assert "UTC" in str(app.screen.query_one("#history-status", Static).render())

            await pilot.press("enter")
            await _settle(pilot, 3)
            assert isinstance(app.screen, ActivityDetailDialog)
            assert app.screen.query_one("#activity-ops", DataTable).row_count == 2
            assert "tx-normal" in str(app.screen.query_one("#activity-detail", Static).render())

            await pilot.click("#add-contact")
            await _settle(pilot, 3)
            assert isinstance(app.screen, AddContactDialog)
            assert app.screen.query_one("#contact-address", Input).value == runtime.sender
            app.screen.query_one("#contact-name", Input).value = "Sender"
            await pilot.click("#add")
            await _settle(pilot, 3)
            assert runtime.contact_store.get("Sender").address == runtime.sender

            assert isinstance(app.screen, HistoryScreen)
            await pilot.press("c")
            await _settle(pilot, 3)
            assert isinstance(app.screen, ContactBookScreen)
            assert app.screen.query_one("#contacts-table", DataTable).row_count == 1

    asyncio.run(scenario())

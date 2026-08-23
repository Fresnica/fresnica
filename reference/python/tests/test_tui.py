import asyncio
from decimal import Decimal
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import Button, DataTable, Input, Label, Select, Static

from fresnica.manager import WalletManager, WalletState
from fresnica.models import (
    Asset,
    BalanceView,
    LiquidityPositionView,
    LiquidityReserveView,
    OperationView,
    TransactionResult,
)
from fresnica.review import TransactionReview
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.history import HistoryScreen
from fresnica.tui.screens import (
    AddWalletDialog,
    ConfirmDialog,
    CreateWalletDialog,
    MnemonicBackupDialog,
    NoticeDialog,
    ReviewDialog,
    SendDialog,
    UnlockDialog,
)
from fresnica.tui.wallet_management import WalletManagerDialog


class FakeAdapter:
    def __init__(self, existing=None):
        self.existing = dict(existing or {})
        self.checks = 0

    def account_exists(self, address):
        self.checks += 1
        return bool(self.existing.get(address, False))


class FakeBalanceService:
    def __init__(self, adapter):
        self.adapter = adapter
        self.cached_accounts = set()
        self.cached_reads = 0

    def get_views(self, wallet):
        return self.get_portfolio_views(wallet)[0]

    def _portfolio(self):
        issuer = Keypair.random().public_key
        return (
            [
                BalanceView(
                    asset=Asset("XLM"),
                    balance=Decimal("10.0000000"),
                    selling_liabilities=Decimal("0E-7"),
                    buying_liabilities=Decimal("0"),
                    available=Decimal("9.0000000"),
                ),
                BalanceView(
                    asset=Asset("USDC", issuer),
                    balance=Decimal("0E-7"),
                    selling_liabilities=Decimal("0"),
                    buying_liabilities=Decimal("0"),
                    available=Decimal("0"),
                    raw={"_fresnica_issuer_domain": "example.org"},
                ),
            ],
            [
                LiquidityPositionView(
                    pool_id="a" * 64,
                    shares=Decimal("4.0000000"),
                    reserves=[
                        LiquidityReserveView(Asset("XLM"), Decimal("2.5")),
                        LiquidityReserveView(Asset("USDC", issuer), Decimal("7.5")),
                    ],
                )
            ],
        )

    def get_portfolio_views(self, wallet):
        self.cached_accounts.add(wallet.address())
        return self._portfolio()

    def get_cached_portfolio_views(self, wallet):
        self.cached_reads += 1
        if wallet.address() not in self.cached_accounts:
            return [], []
        return self._portfolio()

    def has_cached_account(self, wallet):
        return wallet.address() in self.cached_accounts


class FakeHistoryService:
    def __init__(self):
        self.refreshes = 0
        self.older = 0

    def get_views(self, wallet, limit=20, refresh=True):
        if refresh:
            self.refreshes += 1
        records = [
            OperationView(
                operation_type="payment",
                created_at="2026-08-22T12:00:00Z",
                summary="Received 1 XLM from GABC...1234",
            )
        ]
        return records[:limit]

    def get_activity_views(self, wallet, limit=20, refresh=True):
        return self.get_views(wallet, limit=limit, refresh=refresh)

    def load_older(self, wallet, limit=200):
        self.older += 1
        return 0


class FakeTransferService:
    def __init__(self, network):
        self.network = network
        self.signed = False
        self.submitted = False

    def prepare(self, wallet_name, wallet, destination, asset, amount, memo=None):
        return SimpleNamespace(
            review=TransactionReview(
                wallet_name=wallet_name,
                source=wallet.address(),
                destination=destination,
                asset=asset,
                amount=amount,
                fee="0.00001",
                network=self.network,
                memo=memo,
            )
        )

    def sign(self, wallet, prepared):
        self.signed = True

    def submit(self, prepared):
        self.submitted = True
        return TransactionResult(hash="abc123", ledger=42, successful=True)


class FakeTestnetService:
    def __init__(self):
        self.funded = []

    def fund(self, address):
        self.funded.append(address)
        return {"hash": "friendbot123"}


class FakeRuntime:
    def __init__(self):
        self.network = "mainnet"
        storage = MemoryWalletStorage()
        self.wallet_manager = WalletManager(storage)
        self.secret = Keypair.random()
        self.watch = Keypair.random()
        self.wallet_manager.import_secret(
            "alpha",
            self.secret.secret,
            "pw",
            network="testnet",
            make_default=True,
        )
        self.wallet_manager.add_watch(
            "beta",
            self.watch.public_key,
            network="mainnet",
            make_default=False,
        )
        self.adapters = {
            "testnet": FakeAdapter({self.secret.public_key: True}),
            "mainnet": FakeAdapter({self.watch.public_key: True}),
        }
        self.balance_services = {
            network: FakeBalanceService(adapter)
            for network, adapter in self.adapters.items()
        }
        self.transfer_services = {
            "testnet": FakeTransferService("testnet"),
            "mainnet": FakeTransferService("mainnet"),
        }
        self.testnet_service = FakeTestnetService()
        self.history_services = {
            "testnet": FakeHistoryService(),
            "mainnet": FakeHistoryService(),
        }

    def services_for(self, network):
        return SimpleNamespace(
            balance_service=self.balance_services[network],
            history_service=self.history_services[network],
            transfer_service=self.transfer_services[network],
            testnet_service=self.testnet_service if network == "testnet" else None,
        )


async def _settle(pilot, rounds=4):
    for _ in range(rounds):
        await pilot.pause(0.03)


def test_dashboard_is_responsive_and_formats_wallet_assets():
    async def wide_scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(140, 44)) as pilot:
            await _settle(pilot, 8)
            assert app.query_one("#dashboard").has_class("wide")
            wallet_text = str(app.query_one("#wallet", Static).render())
            assert "TESTNET" in wallet_text
            assert "Locked" in wallet_text
            assert "LOCKED" not in wallet_text
            assert len(app.query("#wallet-actions")) == 0
            bindings = {binding.key: binding for binding in app.BINDINGS}
            assert bindings["s"].show
            assert bindings["l"].show

            balances = app.query_one("#balances")
            assert balances.row_count == 1
            row = list(balances.get_row_at(0))
            assert row[0] == "XLM"
            assert row[2] == "10"
            assert row[4] == ""
            assert app.query_one("#liquidity").row_count == 1
            assert "Updated" in str(app.query_one("#sync-status", Static).render())

            await pilot.press("z")
            await _settle(pilot)
            assert balances.row_count == 2
            issued_row = list(balances.get_row_at(1))
            assert issued_row[1] == "example.org"
            assert issued_row[4] == ""
            assert "showing zero" in str(app.query_one("#assets-title", Label).render())

    async def narrow_scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(90, 44)) as pilot:
            await _settle(pilot, 8)
            assert not app.query_one("#dashboard").has_class("wide")

    asyncio.run(wide_scenario())
    asyncio.run(narrow_scenario())


def test_tui_shows_wallet_state_and_independent_unlock_lock_flow():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot, 8)

            await pilot.press("l")
            assert isinstance(app.screen, UnlockDialog)
            app.screen.query_one("#unlock-password", Input).value = "wrong"
            await pilot.click("#unlock")
            await _settle(pilot, 8)
            assert "Invalid wallet password" in str(
                app.screen.query_one("#form-error", Static).render()
            )

            app.screen.query_one("#unlock-password", Input).value = "pw"
            await pilot.click("#unlock")
            await _settle(pilot, 8)
            assert runtime.wallet_manager.state("alpha") is WalletState.UNLOCKED

            await pilot.press("l")
            await _settle(pilot, 6)
            assert runtime.wallet_manager.state("alpha") is WalletState.LOCKED

    asyncio.run(scenario())


def test_watch_only_hides_internal_state_and_send_uses_notice():
    async def scenario():
        runtime = FakeRuntime()
        runtime.wallet_manager.set_default("beta")
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot, 8)
            wallet_text = str(app.query_one("#wallet", Static).render())
            assert "Watch-only" in wallet_text
            assert "WATCH_ONLY" not in wallet_text
            await pilot.press("s")
            assert isinstance(app.screen, NoticeDialog)
            assert "cannot sign transactions" in str(
                app.screen.query_one("#message", Static).render()
            )

    asyncio.run(scenario())


def test_history_key_opens_transaction_activity_screen():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot, 8)
            await pilot.press("h")
            assert isinstance(app.screen, HistoryScreen)
            await _settle(pilot, 8)
            assert app.screen.query_one("#history-table").row_count == 1
            assert "Activity" in str(app.screen.query_one("#history-title", Static).render())
            await pilot.press("m")
            await _settle(pilot, 8)
            # Older reveals the retained local cache; it no longer starts a
            # separate Horizon backfill in the default 2,000-operation mode.
            assert runtime.history_services["testnet"].older == 0
            assert "No more cached activity" in str(
                app.screen.query_one("#history-status", Static).render()
            )

    asyncio.run(scenario())


def test_locked_send_unlocks_first_and_wallet_remains_unlocked_after_submit():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot, 8)
            destination = Keypair.random().public_key

            await pilot.press("s")
            assert isinstance(app.screen, UnlockDialog)
            app.screen.query_one("#unlock-password", Input).value = "pw"
            await pilot.click("#unlock")
            await _settle(pilot, 8)

            assert isinstance(app.screen, SendDialog)
            assert len(app.screen.query("#password")) == 0
            app.screen.query_one("#amount", Input).value = "1"
            app.screen.query_one("#asset", Input).value = "XLM"
            app.screen.query_one("#destination", Input).value = destination
            await pilot.click("#review")
            await _settle(pilot, 6)

            assert isinstance(app.screen, ReviewDialog)
            await pilot.click("#confirm")
            await _settle(pilot, 8)

            transfer = runtime.transfer_services["testnet"]
            assert transfer.signed and transfer.submitted
            assert runtime.wallet_manager.state("alpha") is WalletState.UNLOCKED
            assert "abc123" in str(app.query_one("#status", Static).render())

    asyncio.run(scenario())


def test_wallet_management_defers_dashboard_io_until_close():
    async def scenario():
        runtime = FakeRuntime()
        unfunded = Keypair.random()
        runtime.wallet_manager.import_secret(
            "unfunded",
            unfunded.secret,
            "pw",
            network="testnet",
            make_default=False,
        )
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 46)) as pilot:
            await _settle(pilot, 8)
            checks_before = sum(adapter.checks for adapter in runtime.adapters.values())
            testnet_cached_before = runtime.balance_services["testnet"].cached_reads
            mainnet_cached_before = runtime.balance_services["mainnet"].cached_reads
            mainnet_history_before = runtime.history_services["mainnet"].refreshes

            await pilot.press("w")
            assert isinstance(app.screen, WalletManagerDialog)
            await _settle(pilot, 4)
            assert sum(adapter.checks for adapter in runtime.adapters.values()) == checks_before

            table = app.screen.query_one("#wallet-list", DataTable)
            assert table.row_count == 3
            assert app.screen.query_one("#library-section").parent.id == "secondary-sections"
            assert app.screen.query_one("#danger-section").parent.id == "secondary-sections"

            # Alpha has a cached account, so Friendbot is irrelevant and hidden.
            assert app.screen.query_one("#lock", Button).display
            assert not app.screen.query_one("#fund", Button).display
            assert "Testnet account exists" in str(
                app.screen.query_one("#wallet-detail", Static).render()
            )

            # Preview and Enter only change wallet identity and context actions.
            table.move_cursor(row=1)
            await _settle(pilot)
            detail = str(app.screen.query_one("#wallet-detail", Static).render())
            assert "Watch-only" in detail
            assert "WATCH_ONLY" not in detail
            assert not app.screen.query_one("#lock", Button).display
            assert not app.screen.query_one("#fund", Button).display

            await pilot.press("enter")
            await _settle(pilot, 4)
            assert isinstance(app.screen, WalletManagerDialog)
            assert runtime.wallet_manager.get_record().name == "beta"
            assert runtime.balance_services["testnet"].cached_reads == testnet_cached_before
            assert runtime.balance_services["mainnet"].cached_reads == mainnet_cached_before
            assert runtime.history_services["mainnet"].refreshes == mainnet_history_before
            assert sum(adapter.checks for adapter in runtime.adapters.values()) == checks_before

            # Close commits presentation: cache renders immediately, then network refreshes.
            await pilot.click("#close")
            await _settle(pilot, 8)
            assert not isinstance(app.screen, WalletManagerDialog)
            assert runtime.balance_services["mainnet"].cached_reads > mainnet_cached_before
            assert runtime.history_services["mainnet"].refreshes > mainnet_history_before

            # Unknown Testnet status does not trigger a background probe in management.
            checks_before = sum(adapter.checks for adapter in runtime.adapters.values())
            await pilot.press("w")
            table = app.screen.query_one("#wallet-list", DataTable)
            table.move_cursor(row=2)
            await _settle(pilot, 4)
            assert sum(adapter.checks for adapter in runtime.adapters.values()) == checks_before
            assert app.screen.query_one("#fund", Button).display
            assert "status not cached" in str(
                app.screen.query_one("#wallet-detail", Static).render()
            )

            await pilot.press("enter")
            await _settle(pilot, 2)
            assert runtime.wallet_manager.get_record().name == "unfunded"
            assert sum(adapter.checks for adapter in runtime.adapters.values()) == checks_before

            # Fund is an explicit network action: verify existence once, then Friendbot.
            await pilot.click("#fund")
            await _settle(pilot, 8)
            assert runtime.adapters["testnet"].checks >= 1
            assert runtime.testnet_service.funded == [unfunded.public_key]

            await pilot.press("w")
            table = app.screen.query_one("#wallet-list", DataTable)
            table.move_cursor(row=1)
            await _settle(pilot)
            await pilot.click("#delete")
            assert isinstance(app.screen, ConfirmDialog)
            await pilot.click("#confirm")
            await _settle(pilot, 6)
            assert [record.name for record in runtime.wallet_manager.list_wallets()] == [
                "alpha",
                "unfunded",
            ]

    asyncio.run(scenario())


def test_wallet_management_can_create_wallet_and_show_backup_once():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 45)) as pilot:
            await _settle(pilot, 8)
            await pilot.press("w")
            assert isinstance(app.screen, WalletManagerDialog)
            await pilot.click("#add")
            assert isinstance(app.screen, AddWalletDialog)
            app.screen.query_one("#add-kind", Select).value = "create"
            await pilot.click("#continue")
            assert isinstance(app.screen, CreateWalletDialog)

            app.screen.query_one("#name", Input).value = "gamma"
            app.screen.query_one("#network", Select).value = "testnet"
            app.screen.query_one("#strength", Select).value = "128"
            app.screen.query_one("#password", Input).value = "gamma-pw"
            app.screen.query_one("#password-confirm", Input).value = "gamma-pw"
            await pilot.click("#create")
            await _settle(pilot, 20)

            assert isinstance(app.screen, MnemonicBackupDialog)
            assert runtime.wallet_manager.get_record().name == "gamma"
            assert runtime.wallet_manager.state("gamma") is WalletState.LOCKED
            mnemonic = str(app.screen.query_one("#mnemonic", Static).render())
            assert len(mnemonic.split()) >= 12

    asyncio.run(scenario())

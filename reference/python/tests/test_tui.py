import asyncio
from decimal import Decimal
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import Button, Input, Select, Static

from fresnica.manager import WalletManager, WalletState
from fresnica.models import Asset, BalanceView, OperationView, TransactionResult
from fresnica.review import TransactionReview
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.screens import (
    AddWalletDialog,
    ConfirmDialog,
    CreateWalletDialog,
    MnemonicBackupDialog,
    NoticeDialog,
    ReviewDialog,
    SendDialog,
    UnlockDialog,
    WalletManagerDialog,
)


class FakeBalanceService:
    def get_views(self, wallet):
        return [
            BalanceView(
                asset=Asset("XLM"),
                balance=Decimal("10"),
                selling_liabilities=Decimal("0"),
                buying_liabilities=Decimal("0"),
                available=Decimal("9"),
            )
        ]


class FakeHistoryService:
    def get_views(self, wallet, limit=20):
        return [
            OperationView(
                operation_type="payment",
                created_at="2026-08-22T12:00:00Z",
                summary="1 XLM sent",
            )
        ]


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
        self.transfer_services = {
            "testnet": FakeTransferService("testnet"),
            "mainnet": FakeTransferService("mainnet"),
        }
        self.testnet_service = FakeTestnetService()

    def services_for(self, network):
        return SimpleNamespace(
            balance_service=FakeBalanceService(),
            history_service=FakeHistoryService(),
            transfer_service=self.transfer_services[network],
            testnet_service=self.testnet_service if network == "testnet" else None,
        )


async def _settle(pilot, rounds=4):
    for _ in range(rounds):
        await pilot.pause(0.03)


def test_tui_shows_wallet_state_and_independent_unlock_lock_flow():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot)
            wallet_text = str(app.query_one("#wallet", Static).render())
            assert "alpha" in wallet_text
            assert "TESTNET" in wallet_text
            assert "LOCKED" in wallet_text
            assert "L Unlock" in str(app.query_one("#actions", Static).render())

            await pilot.press("l")
            assert isinstance(app.screen, UnlockDialog)
            app.screen.query_one("#unlock-password", Input).value = "wrong"
            await pilot.click("#unlock")
            await _settle(pilot, 8)
            assert isinstance(app.screen, UnlockDialog)
            assert "Invalid wallet password" in str(
                app.screen.query_one("#form-error", Static).render()
            )

            app.screen.query_one("#unlock-password", Input).value = "pw"
            await pilot.click("#unlock")
            await _settle(pilot, 8)
            assert runtime.wallet_manager.state("alpha") is WalletState.UNLOCKED
            assert "UNLOCKED" in str(app.query_one("#wallet", Static).render())
            assert "L Lock" in str(app.query_one("#actions", Static).render())

            await pilot.press("l")
            await _settle(pilot)
            assert runtime.wallet_manager.state("alpha") is WalletState.LOCKED
            assert "LOCKED" in str(app.query_one("#wallet", Static).render())

    asyncio.run(scenario())


def test_watch_only_send_uses_capability_notice_not_status_error():
    async def scenario():
        runtime = FakeRuntime()
        runtime.wallet_manager.set_default("beta")
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot)
            assert "WATCH_ONLY" in str(app.query_one("#wallet", Static).render())
            assert "S Send" not in str(app.query_one("#actions", Static).render())

            await pilot.press("s")
            assert isinstance(app.screen, NoticeDialog)
            message = str(app.screen.query_one("#message", Static).render())
            assert "cannot sign transactions" in message
            assert "ERROR" not in str(app.query_one("#status", Static).render())

    asyncio.run(scenario())


def test_locked_send_unlocks_first_and_wallet_remains_unlocked_after_submit():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot)
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
            assert runtime.wallet_manager.state("alpha") is WalletState.UNLOCKED
            await pilot.click("#confirm")
            await _settle(pilot, 8)

            transfer = runtime.transfer_services["testnet"]
            assert transfer.signed
            assert transfer.submitted
            assert runtime.wallet_manager.state("alpha") is WalletState.UNLOCKED
            assert "abc123" in str(app.query_one("#status", Static).render())

    asyncio.run(scenario())


def test_wallet_management_is_context_aware_and_supports_fund_delete():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot)
            await pilot.press("w")
            assert isinstance(app.screen, WalletManagerDialog)

            select = app.screen.query_one("#wallet-select", Select)
            select.value = "beta"
            await _settle(pilot)
            assert app.screen.query_one("#lock", Button).disabled
            assert app.screen.query_one("#fund", Button).disabled

            select.value = "alpha"
            await _settle(pilot)
            assert not app.screen.query_one("#lock", Button).disabled
            assert not app.screen.query_one("#fund", Button).disabled
            await pilot.press("f")
            await _settle(pilot, 8)
            assert runtime.testnet_service.funded == [runtime.secret.public_key]
            assert "friendbot123" in str(app.query_one("#status", Static).render())

            await pilot.press("w")
            app.screen.query_one("#wallet-select", Select).value = "beta"
            await _settle(pilot)
            await pilot.press("d")
            assert isinstance(app.screen, ConfirmDialog)
            await pilot.click("#confirm")
            await _settle(pilot)
            assert [record.name for record in runtime.wallet_manager.list_wallets()] == ["alpha"]

    asyncio.run(scenario())


def test_wallet_management_can_create_wallet_and_show_backup_once():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 45)) as pilot:
            await _settle(pilot)
            await pilot.press("w")
            assert isinstance(app.screen, WalletManagerDialog)
            await pilot.press("a")
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
            await pilot.press("enter")
            await _settle(pilot, 6)
            assert "gamma" in str(app.query_one("#wallet", Static).render())
            assert "LOCKED" in str(app.query_one("#wallet", Static).render())

    asyncio.run(scenario())

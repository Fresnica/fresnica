import asyncio
from decimal import Decimal
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import Input, Select, Static

from fresnica.manager import WalletManager
from fresnica.models import Asset, BalanceView, OperationView, TransactionResult
from fresnica.review import TransactionReview
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.screens import (
    ReviewDialog,
    SendDialog,
    WalletManagerDialog,
    WatchWalletDialog,
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

    def services_for(self, network):
        return SimpleNamespace(
            balance_service=FakeBalanceService(),
            history_service=FakeHistoryService(),
            transfer_service=self.transfer_services[network],
        )


async def _settle(pilot, rounds=3):
    for _ in range(rounds):
        await pilot.pause(0.02)


def test_tui_wallet_management_switch_shortcut_and_cancel():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 40)) as pilot:
            await _settle(pilot)
            assert "alpha" in str(app.query_one("#wallet", Static).render())
            assert app.query_one("#history").row_count == 1

            await pilot.press("w")
            assert isinstance(app.screen, WalletManagerDialog)
            app.screen.query_one("#wallet-select", Select).value = "beta"
            await pilot.press("escape")
            await _settle(pilot)
            assert runtime.wallet_manager.get_record().name == "alpha"

            await pilot.press("w")
            assert isinstance(app.screen, WalletManagerDialog)
            app.screen.query_one("#wallet-select", Select).value = "beta"
            await pilot.press("s")
            await _settle(pilot)

            assert runtime.wallet_manager.get_record().name == "beta"
            assert "beta" in str(app.query_one("#wallet", Static).render())

    asyncio.run(scenario())


def test_tui_wallet_management_add_watch_only():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        address = Keypair.random().public_key
        async with app.run_test(size=(120, 40)) as pilot:
            await _settle(pilot)
            await pilot.press("w")
            assert isinstance(app.screen, WalletManagerDialog)
            await pilot.press("a")
            assert isinstance(app.screen, WatchWalletDialog)

            app.screen.query_one("#watch-name", Input).value = "gamma"
            app.screen.query_one("#watch-address", Input).value = address
            app.screen.query_one("#watch-network", Select).value = "mainnet"
            await pilot.click("#add")
            await _settle(pilot)

            record = runtime.wallet_manager.get_record("gamma")
            assert record.watch_only
            assert record.address == address
            assert record.network == "mainnet"
            assert "Added watch-only wallet" in str(app.query_one("#status", Static).render())

    asyncio.run(scenario())


def test_tui_send_prepare_review_submit_and_lock():
    async def scenario():
        runtime = FakeRuntime()
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 40)) as pilot:
            await _settle(pilot)
            destination = Keypair.random().public_key

            await pilot.press("s")
            assert isinstance(app.screen, SendDialog)
            app.screen.query_one("#amount", Input).value = "1"
            app.screen.query_one("#asset", Input).value = "XLM"
            app.screen.query_one("#destination", Input).value = destination
            app.screen.query_one("#password", Input).value = "pw"
            await pilot.click("#review")
            await _settle(pilot, 5)

            assert isinstance(app.screen, ReviewDialog)
            assert runtime.wallet_manager.current() is not None
            await pilot.click("#confirm")
            await _settle(pilot, 5)

            transfer = runtime.transfer_services["testnet"]
            assert transfer.signed
            assert transfer.submitted
            assert runtime.wallet_manager.current() is None
            assert "abc123" in str(app.query_one("#status", Static).render())

    asyncio.run(scenario())

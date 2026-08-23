import asyncio
from types import SimpleNamespace

from stellar_sdk import Keypair
from textual.widgets import Input, Static

from fresnica.contacts import ContactStore
from fresnica.manager import WalletManager
from fresnica.models import TransactionResult
from fresnica.review import TransactionReview
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.app import FresnicaApp
from fresnica.tui.review_dialog import ReviewPresentationDialog
from fresnica.tui.screens import SendDialog


class BalanceService:
    def get_portfolio_views(self, wallet):
        return [], []

    def get_cached_portfolio_views(self, wallet):
        return [], []

    def has_cached_account(self, wallet):
        return True


class HistoryService:
    def get_activity_views(self, wallet, limit=20, refresh=True):
        return []

    get_views = get_activity_views


class CapturingTransferService:
    def __init__(self):
        self.prepared = None

    def prepare(
        self,
        wallet_name,
        wallet,
        destination,
        asset,
        amount,
        memo=None,
        contact_name=None,
    ):
        self.prepared = {
            "wallet_name": wallet_name,
            "destination": destination,
            "asset": asset,
            "amount": amount,
            "memo": memo,
            "contact_name": contact_name,
        }
        return SimpleNamespace(
            review=TransactionReview(
                wallet_name=wallet_name,
                source=wallet.address(),
                destination=destination,
                asset=asset,
                amount=amount,
                fee="0.00001",
                network="mainnet",
                memo=memo,
                contact_name=contact_name,
            )
        )

    def sign(self, wallet, prepared):
        return prepared

    def submit(self, prepared):
        return TransactionResult(hash="unused", ledger=1, successful=True)


class Runtime:
    def __init__(self, tmp_path):
        self.network = "mainnet"
        self.settings = SimpleNamespace(show_zero_balances=False)
        self.settings_store = SimpleNamespace(save=lambda settings: None)
        self.contact_store = ContactStore(tmp_path / "contacts.json")
        self.destination = Keypair.random().public_key
        self.contact_store.add("Alice", self.destination, memo="contact-memo")

        self.wallet_manager = WalletManager(MemoryWalletStorage())
        signer = Keypair.random()
        self.wallet_manager.import_secret(
            "main",
            signer.secret,
            "pw",
            network="mainnet",
            make_default=True,
        )
        self.wallet_manager.unlock("main", "pw")

        self.transfer_service = CapturingTransferService()
        self.services = SimpleNamespace(
            balance_service=BalanceService(),
            history_service=HistoryService(),
            transfer_service=self.transfer_service,
            pending_transaction_service=None,
            testnet_service=None,
        )

    def services_for(self, network=None):
        return self.services


async def _settle(pilot, rounds=6):
    for _ in range(rounds):
        await pilot.pause(0.03)


def test_tui_send_resolves_contact_and_shows_identity_in_shared_review(tmp_path):
    async def scenario():
        runtime = Runtime(tmp_path)
        app = FresnicaApp(runtime)
        async with app.run_test(size=(120, 42)) as pilot:
            await _settle(pilot, 8)
            await pilot.press("s")
            assert isinstance(app.screen, SendDialog)

            app.screen.query_one("#amount", Input).value = "2"
            app.screen.query_one("#asset", Input).value = "XLM"
            app.screen.query_one("#destination", Input).value = "alice"
            await pilot.click("#review")
            await _settle(pilot, 8)

            assert runtime.transfer_service.prepared == {
                "wallet_name": "main",
                "destination": runtime.destination,
                "asset": "XLM",
                "amount": "2",
                "memo": "contact-memo",
                "contact_name": "Alice",
            }
            assert isinstance(app.screen, ReviewPresentationDialog)
            review_text = str(app.screen.query_one("#review-text", Static).render())
            assert f"To: Alice ({runtime.destination})" in review_text
            assert "Memo: contact-memo" in review_text

    asyncio.run(scenario())

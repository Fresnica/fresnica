import asyncio
from decimal import Decimal

from stellar_sdk import Keypair
from textual.app import App
from textual.widgets import Input, Static

from fresnica.anchor_service import AnchorCapabilities, AnchorInteractiveTransfer
from fresnica.balance_service import ISSUER_DOMAIN_CACHE_KEY
from fresnica.manager import WalletManager
from fresnica.models import Asset, BalanceView
from fresnica.storage import MemoryWalletStorage
from fresnica.tui.asset_details import AssetDetailsScreen
from fresnica.tui.trustlines import TrustlineFormDialog


async def _settle(pilot, rounds=6):
    for _ in range(rounds):
        await pilot.pause(0.04)


class Runtime:
    def __init__(self):
        self.wallet_manager = WalletManager(MemoryWalletStorage())
        self.keypair = Keypair.random()
        self.wallet_manager.import_secret(
            "main",
            self.keypair.secret,
            "pw",
            network="testnet",
            make_default=True,
        )


class HostApp(App[None]):
    def __init__(self, runtime):
        super().__init__()
        self.runtime = runtime


def _balance():
    issuer = Keypair.random().public_key
    return BalanceView(
        asset=Asset("USD", issuer),
        balance=Decimal("10"),
        selling_liabilities=Decimal("0"),
        buying_liabilities=Decimal("0"),
        available=Decimal("10"),
        raw={
            "asset_type": "credit_alphanum4",
            "asset_code": "USD",
            "asset_issuer": issuer,
            "limit": "100.0000000",
            ISSUER_DOMAIN_CACHE_KEY: "anchor.example",
        },
    )


def test_asset_details_has_direct_trustline_actions_and_no_generic_receive():
    async def scenario():
        runtime = Runtime()
        actions = []
        app = HostApp(runtime)
        balance = _balance()
        async with app.run_test(size=(120, 36)) as pilot:
            screen = AssetDetailsScreen(
                balance,
                runtime=runtime,
                on_trustline_action=lambda source, action: actions.append((source, action)),
            )
            app.push_screen(screen)
            await _settle(pilot)

            assert len(screen.query("#receive")) == 0
            assert len(screen.query("#set-limit")) == 1
            assert len(screen.query("#remove-trustline")) == 1

            await pilot.press("l")
            await _settle(pilot)
            assert isinstance(app.screen, TrustlineFormDialog)
            assert str(app.screen.query_one("#asset-label", Static).render()) == (
                f"USD:{balance.asset.issuer}"
            )
            app.screen.query_one("#limit", Input).value = "250"
            await pilot.click("#review")
            await _settle(pilot)
            assert actions[-1][0] is screen
            assert actions[-1][1].kind == "limit"
            assert actions[-1][1].asset == f"USD:{balance.asset.issuer}"
            assert actions[-1][1].limit == "250"

            await pilot.press("x")
            await _settle(pilot)
            assert actions[-1][1].kind == "remove"
            assert actions[-1][1].asset == f"USD:{balance.asset.issuer}"

    asyncio.run(scenario())


def test_anchor_discovery_exposes_real_sep24_action_and_opens_browser(monkeypatch):
    async def scenario():
        runtime = Runtime()
        runtime.wallet_manager.unlock("main", "pw")
        opened = []
        started = []
        balance = _balance()
        capabilities = AnchorCapabilities(
            domain="anchor.example",
            sep24_url="https://anchor.example/sep24",
            web_auth_url="https://anchor.example/auth",
            signing_key=Keypair.random().public_key,
            sep24_deposit=True,
            sep24_withdraw=True,
        )

        class FakeAnchorService:
            def discover(self, asset, domain):
                assert asset == balance.asset
                assert domain == "anchor.example"
                return capabilities

            def start_sep24(self, wallet, asset, found, kind, network_passphrase):
                started.append((wallet.address(), asset, found, kind, network_passphrase))
                return AnchorInteractiveTransfer(
                    kind=kind,
                    url="https://anchor.example/interactive/1",
                    transaction_id="1",
                )

        monkeypatch.setattr("fresnica.tui.asset_details.AnchorService", FakeAnchorService)
        monkeypatch.setattr(
            "fresnica.tui.asset_details.webbrowser.open",
            lambda url, new=0: opened.append((url, new)) or True,
        )

        app = HostApp(runtime)
        async with app.run_test(size=(120, 36)) as pilot:
            screen = AssetDetailsScreen(balance, runtime=runtime)
            app.push_screen(screen)
            await _settle(pilot)
            assert screen.query_one("#anchor-deposit").display is False

            await pilot.press("a")
            await _settle(pilot, 8)
            assert screen.query_one("#anchor-deposit").display is True
            assert screen.query_one("#anchor-withdraw").display is True

            await pilot.press("d")
            await _settle(pilot, 8)
            assert started and started[0][3] == "deposit"
            assert opened == [("https://anchor.example/interactive/1", 2)]
            assert "Opened anchor deposit flow" in str(
                screen.query_one("#asset-status", Static).render()
            )

    asyncio.run(scenario())

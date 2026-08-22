from decimal import Decimal
from types import SimpleNamespace

from rich.console import Console
from stellar_sdk import Keypair

from fresnica.cli.rich_renderer import RichRenderer
from fresnica.models import Asset, BalanceView, OperationView


def test_cli_balance_and_history_use_human_portfolio_activity_semantics():
    issuer = Keypair.random().public_key
    record = SimpleNamespace(
        name="observer",
        address=Keypair.random().public_key,
        network="mainnet",
    )
    balances = [
        BalanceView(
            asset=Asset("XLM"),
            balance=Decimal("10.0000000"),
            selling_liabilities=Decimal("0E-7"),
            available=Decimal("9.0000000"),
        ),
        BalanceView(
            asset=Asset("USDC", issuer),
            balance=Decimal("25.0000000"),
            selling_liabilities=Decimal("5.0000000"),
            available=Decimal("20.0000000"),
        ),
    ]
    console = Console(record=True, width=140)
    renderer = RichRenderer(console)

    renderer.render_balance(record, balances)
    balance_text = console.export_text(clear=True)
    assert "Issuer / source" in balance_text
    assert "In offers" in balance_text
    assert "0E-7" not in balance_text
    assert "10" in balance_text
    assert "USDC" in balance_text
    assert issuer[:6] in balance_text

    renderer.render_history(
        record,
        [
            OperationView(
                operation_type="payment",
                created_at="2026-08-22T12:00:00Z",
                summary="Received 1 XLM from GABC...1234",
            )
        ],
    )
    history_text = console.export_text()
    assert "Activity" in history_text
    assert "Received 1 XLM" in history_text

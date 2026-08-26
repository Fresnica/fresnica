from decimal import Decimal
from types import SimpleNamespace

from rich.console import Console
from stellar_sdk import Keypair

from fresnica.cli.rich_renderer import RichRenderer
from fresnica.models import Asset, BalanceView, OperationView
from fresnica.review import OfferReview, TransactionReview


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


def test_offer_review_uses_shared_risk_semantics():
    console = Console(record=True, width=160)
    renderer = RichRenderer(console)
    renderer.render_offer_review(
        OfferReview(
            wallet_name="main",
            source=Keypair.random().public_key,
            action="create",
            side="buy",
            base_asset="XRP:GXRP",
            counter_asset="XLM",
            amount="2",
            price="1.5",
            total="3",
            fee="0.00002",
            network="testnet",
            trustline_asset="XRP:GXRP",
            trustline_limit="708269837873.6765",
        )
    )

    text = console.export_text()
    assert "Create BUY limit offer" in text
    assert "Max spend: 3 XLM" in text
    assert "Warning: Creates trustline for XRP:GXRP" in text
    assert "Fee: 0.00002 XLM" in text
    assert "Total:" not in text


def test_transfer_review_does_not_show_core_operation_name():
    console = Console(record=True, width=160)
    renderer = RichRenderer(console)
    renderer.render_review(
        TransactionReview(
            wallet_name="main",
            source="GSOURCE",
            destination="GDESTINATION",
            asset="USDC:GISSUER",
            amount="2",
            fee="0.00001",
            network="mainnet",
            operation="payment",
        )
    )

    text = console.export_text()
    assert "Send 2 USDC:GISSUER" in text
    assert "Amount: 2 USDC:GISSUER" in text
    assert "Operation:" not in text
    assert "Payment" not in text


def test_trade_renderer_prefers_horizon_price_fraction_over_rounded_amount_ratio():
    console = Console(record=True, width=120)
    renderer = RichRenderer(console)
    renderer.render_trades(
        "USDC:GISSUER",
        "XLM",
        [
            {
                "ledger_close_time": "2026-08-23T00:00:00Z",
                "base_amount": "0.0131638",
                "counter_amount": "0.0781235",
                "price": {"n": 2000, "d": 337},
                "base_is_seller": True,
            }
        ],
        "mainnet",
    )

    text = console.export_text()
    assert "5.9347181" in text
    assert "5.9347224" not in text

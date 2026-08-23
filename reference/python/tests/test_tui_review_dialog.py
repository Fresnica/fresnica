from types import SimpleNamespace

from fresnica.review import TransactionReview
from fresnica.tui.app import FresnicaApp
from fresnica.tui.review_dialog import ReviewPresentationDialog


class AppProbe:
    def __init__(self):
        self.status = None
        self.pushed = None
        self._pending_send = None

    def _set_status(self, message):
        self.status = message

    def _on_review(self, confirmed):
        return confirmed

    def push_screen(self, screen, callback):
        self.pushed = (screen, callback)


def test_product_payment_review_routes_through_shared_presentation_dialog():
    review = TransactionReview(
        wallet_name="main",
        source="GSOURCE",
        destination="GDEST",
        asset="XLM",
        amount="2",
        fee="0.00001",
        network="testnet",
        memo="invoice-7",
        contact_name="Alice",
    )
    prepared = SimpleNamespace(review=review)
    probe = AppProbe()

    FresnicaApp._show_review(
        probe,
        wallet="wallet",
        services="services",
        prepared=prepared,
        network="testnet",
    )

    dialog, callback = probe.pushed
    assert isinstance(dialog, ReviewPresentationDialog)
    assert dialog.presentation.kind == "transfer"
    assert dialog.presentation.title == "Confirm transfer"
    assert "Alice (GDEST)" in dialog.presentation_text
    assert "Memo: invoice-7" in dialog.presentation_text
    assert callback.__self__ is probe
    assert probe._pending_send == ("wallet", "services", prepared, "testnet")

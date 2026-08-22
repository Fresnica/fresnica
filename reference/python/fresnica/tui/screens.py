"""Modal workflows used by the Textual wallet shell."""

from dataclasses import dataclass

from textual.app import ComposeResult
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, Input, Label, Select, Static


@dataclass(frozen=True)
class SendRequest:
    amount: str
    asset: str
    destination: str
    memo: str | None
    password: str


@dataclass(frozen=True)
class WalletAction:
    action: str
    wallet_name: str | None = None


@dataclass(frozen=True)
class WatchWalletRequest:
    name: str
    address: str
    network: str


class WalletManagerDialog(ModalScreen[WalletAction | None]):
    BINDINGS = [
        ("escape", "cancel", "Close"),
        ("s", "switch", "Switch"),
        ("a", "add_watch", "Add watch"),
    ]

    CSS = """
    WalletManagerDialog { align: center middle; }
    WalletManagerDialog > #dialog { width: 76; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    WalletManagerDialog Select { width: 1fr; margin: 1 0; }
    WalletManagerDialog #shortcut-help { margin-bottom: 1; color: $text-muted; }
    WalletManagerDialog #actions { height: auto; align-horizontal: right; }
    WalletManagerDialog Button { margin-left: 1; }
    """

    def __init__(self, records, current_name: str | None):
        super().__init__()
        self.records = list(records)
        self.current_name = current_name

    def compose(self) -> ComposeResult:
        options = [
            (f"{record.name}  [{record.network}]  {record.wallet_type}", record.name)
            for record in self.records
        ]
        value = self.current_name if self.current_name else options[0][1]
        with Vertical(id="dialog"):
            yield Label("Wallet management")
            yield Select(options, value=value, allow_blank=False, id="wallet-select")
            yield Static("S: switch   A: add watch-only   Esc: close", id="shortcut-help")
            with Horizontal(id="actions"):
                yield Button("Add watch-only", id="add-watch")
                yield Button("Close", id="cancel")
                yield Button("Switch", id="switch", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def action_switch(self) -> None:
        value = self.query_one("#wallet-select", Select).value
        self.dismiss(WalletAction("switch", str(value)))

    def action_add_watch(self) -> None:
        self.dismiss(WalletAction("add-watch"))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
        elif event.button.id == "switch":
            self.action_switch()
        elif event.button.id == "add-watch":
            self.action_add_watch()


class WatchWalletDialog(ModalScreen[WatchWalletRequest | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    WatchWalletDialog { align: center middle; }
    WatchWalletDialog > #dialog { width: 82; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    WatchWalletDialog Input, WatchWalletDialog Select { margin-top: 1; }
    WatchWalletDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    WatchWalletDialog Button { margin-left: 1; }
    """

    def __init__(self, network: str):
        super().__init__()
        self.network = network

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Add watch-only wallet")
            yield Input(placeholder="Wallet name", id="watch-name")
            yield Input(placeholder="Stellar G... address", id="watch-address")
            yield Select(
                [("Mainnet", "mainnet"), ("Testnet", "testnet")],
                value=self.network,
                allow_blank=False,
                id="watch-network",
            )
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Add", id="add", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "add":
            return

        name = self.query_one("#watch-name", Input).value.strip()
        address = self.query_one("#watch-address", Input).value.strip()
        network = str(self.query_one("#watch-network", Select).value)
        if not name or not address:
            self.query_one("#form-error", Static).update("Wallet name and address are required.")
            return
        self.dismiss(WatchWalletRequest(name, address, network))


class SendDialog(ModalScreen[SendRequest | None]):
    CSS = """
    SendDialog { align: center middle; }
    SendDialog > #dialog { width: 82; height: auto; max-height: 90%; padding: 1 2; border: round $accent; background: $surface; }
    SendDialog Input { margin-top: 1; }
    SendDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    SendDialog Button { margin-left: 1; }
    """

    def __init__(self, wallet_name: str):
        super().__init__()
        self.wallet_name = wallet_name

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(f"Send from {self.wallet_name}")
            yield Input(placeholder="Amount", id="amount")
            yield Input(value="XLM", placeholder="Asset (XLM or CODE:GISSUER...)", id="asset")
            yield Input(placeholder="Destination G...", id="destination")
            yield Input(placeholder="Memo (optional)", id="memo")
            yield Input(placeholder="Wallet password", password=True, id="password")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Review", id="review", variant="primary")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.dismiss(None)
            return
        if event.button.id != "review":
            return

        amount = self.query_one("#amount", Input).value.strip()
        asset = self.query_one("#asset", Input).value.strip()
        destination = self.query_one("#destination", Input).value.strip()
        memo = self.query_one("#memo", Input).value.strip()
        password = self.query_one("#password", Input).value
        if not amount or not asset or not destination or not password:
            self.query_one("#form-error", Static).update(
                "Amount, asset, destination, and wallet password are required."
            )
            return

        self.dismiss(
            SendRequest(
                amount=amount,
                asset=asset,
                destination=destination,
                memo=memo or None,
                password=password,
            )
        )


class ReviewDialog(ModalScreen[bool]):
    CSS = """
    ReviewDialog { align: center middle; }
    ReviewDialog > #dialog { width: 82; height: auto; padding: 1 2; border: round $warning; background: $surface; }
    ReviewDialog #review-text { margin: 1 0; }
    ReviewDialog #actions { height: auto; align-horizontal: right; }
    ReviewDialog Button { margin-left: 1; }
    """

    def __init__(self, review):
        super().__init__()
        self.review = review

    def compose(self) -> ComposeResult:
        operation = "CreateAccount" if self.review.operation == "create_account" else "Payment"
        memo = f"\nMemo: {self.review.memo}" if self.review.memo else ""
        text = (
            f"Operation: {operation}\n"
            f"From: {self.review.wallet_name} ({self.review.source})\n"
            f"To: {self.review.destination}\n"
            f"Amount: {self.review.amount} {self.review.asset}\n"
            f"Fee: {self.review.fee} XLM\n"
            f"Network: {self.review.network}{memo}"
        )
        with Vertical(id="dialog"):
            yield Label("Confirm transaction")
            yield Static(text, id="review-text")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Confirm", id="confirm", variant="warning")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(event.button.id == "confirm")

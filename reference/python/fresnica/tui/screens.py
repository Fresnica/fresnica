"""State-driven modal workflows used by the Textual wallet shell."""

from dataclasses import dataclass

from textual.app import ComposeResult
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, Input, Label, Select, Static

from ..hdwallet import SUPPORTED_LANGUAGES
from ..manager import WalletState


NETWORK_OPTIONS = [("Mainnet", "mainnet"), ("Testnet", "testnet")]
LANGUAGE_OPTIONS = [
    (language.name.replace("_", " ").title(), language.value)
    for language in SUPPORTED_LANGUAGES
]


@dataclass(frozen=True)
class SendRequest:
    amount: str
    asset: str
    destination: str
    memo: str | None


@dataclass(frozen=True)
class WalletAction:
    action: str
    wallet_name: str | None = None


@dataclass(frozen=True)
class WatchWalletRequest:
    name: str
    address: str
    network: str


@dataclass(frozen=True)
class CreateWalletRequest:
    name: str
    network: str
    language: str
    strength: int
    index: int
    mnemonic_passphrase: str
    password: str


@dataclass(frozen=True)
class ImportSecretRequest:
    name: str
    network: str
    secret: str
    password: str


@dataclass(frozen=True)
class ImportMnemonicRequest:
    name: str
    network: str
    mnemonic: str
    language: str | None
    index: int
    mnemonic_passphrase: str
    password: str


class WalletManagerDialog(ModalScreen[WalletAction | None]):
    BINDINGS = [
        ("escape", "cancel", "Close"),
        ("u", "use", "Use"),
        ("a", "add", "Add"),
        ("l", "toggle_lock", "Lock / Unlock"),
        ("f", "fund", "Fund"),
        ("d", "delete", "Delete"),
    ]

    CSS = """
    WalletManagerDialog { align: center middle; }
    WalletManagerDialog > #dialog { width: 86; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    WalletManagerDialog Select { width: 1fr; margin: 1 0; }
    WalletManagerDialog #wallet-detail { margin-bottom: 1; }
    WalletManagerDialog #shortcut-help { margin-bottom: 1; color: $text-muted; }
    WalletManagerDialog #actions { height: auto; align-horizontal: right; }
    WalletManagerDialog Button { margin-left: 1; }
    """

    def __init__(self, records, current_name: str | None, states: dict[str, WalletState]):
        super().__init__()
        self.records = list(records)
        self.current_name = current_name
        self.states = states

    def compose(self) -> ComposeResult:
        options = [
            (f"{record.name}  [{record.network}]  {record.wallet_type}", record.name)
            for record in self.records
        ]
        value = self.current_name if self.current_name else options[0][1]
        with Vertical(id="dialog"):
            yield Label("Wallet management")
            yield Select(options, value=value, allow_blank=False, id="wallet-select")
            yield Static("", id="wallet-detail")
            yield Static(
                "U: use   A: add   L: lock/unlock   F: testnet fund   D: delete   Esc: close",
                id="shortcut-help",
            )
            with Horizontal(id="actions"):
                yield Button("Add wallet", id="add")
                yield Button("Lock / Unlock", id="lock")
                yield Button("Fund testnet", id="fund")
                yield Button("Delete", id="delete", variant="error")
                yield Button("Close", id="cancel")
                yield Button("Use", id="use", variant="primary")

    def on_mount(self) -> None:
        self._refresh_selection()

    def on_select_changed(self, event: Select.Changed) -> None:
        if event.select.id == "wallet-select":
            self._refresh_selection()

    def _selected_name(self) -> str:
        return str(self.query_one("#wallet-select", Select).value)

    def _selected_record(self):
        name = self._selected_name()
        return next(record for record in self.records if record.name == name)

    def _refresh_selection(self) -> None:
        record = self._selected_record()
        state = self.states[record.name]
        active = "ACTIVE" if record.name == self.current_name else ""
        self.query_one("#wallet-detail", Static).update(
            f"{record.address}\n{record.network.upper()} · {record.wallet_type} · {state.value} {active}".rstrip()
        )
        self.query_one("#lock", Button).disabled = state is WalletState.WATCH_ONLY
        self.query_one("#fund", Button).disabled = record.network != "testnet"

    def action_cancel(self) -> None:
        self.dismiss(None)

    def action_use(self) -> None:
        self.dismiss(WalletAction("use", self._selected_name()))

    def action_add(self) -> None:
        self.dismiss(WalletAction("add"))

    def action_toggle_lock(self) -> None:
        name = self._selected_name()
        state = self.states[name]
        if state is WalletState.WATCH_ONLY:
            return
        self.dismiss(WalletAction("lock" if state is WalletState.UNLOCKED else "unlock", name))

    def action_fund(self) -> None:
        record = self._selected_record()
        if record.network == "testnet":
            self.dismiss(WalletAction("fund", record.name))

    def action_delete(self) -> None:
        self.dismiss(WalletAction("delete", self._selected_name()))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        actions = {
            "cancel": self.action_cancel,
            "use": self.action_use,
            "add": self.action_add,
            "lock": self.action_toggle_lock,
            "fund": self.action_fund,
            "delete": self.action_delete,
        }
        action = actions.get(event.button.id)
        if action is not None:
            action()


class AddWalletDialog(ModalScreen[str | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    AddWalletDialog { align: center middle; }
    AddWalletDialog > #dialog { width: 68; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    AddWalletDialog Select { width: 1fr; margin: 1 0; }
    AddWalletDialog #actions { height: auto; align-horizontal: right; }
    AddWalletDialog Button { margin-left: 1; }
    """

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Add wallet")
            yield Select(
                [
                    ("Create new mnemonic wallet", "create"),
                    ("Import Stellar secret", "import-secret"),
                    ("Import mnemonic", "import-mnemonic"),
                    ("Add watch-only account", "import-watch"),
                ],
                value="create",
                allow_blank=False,
                id="add-kind",
            )
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Continue", id="continue", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def action_continue_add(self) -> None:
        self.dismiss(str(self.query_one("#add-kind", Select).value))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
        elif event.button.id == "continue":
            self.action_continue_add()


class UnlockDialog(ModalScreen[str | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    UnlockDialog { align: center middle; }
    UnlockDialog > #dialog { width: 62; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    UnlockDialog Input { margin-top: 1; }
    UnlockDialog #form-error { color: $error; margin-top: 1; }
    UnlockDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    UnlockDialog Button { margin-left: 1; }
    """

    def __init__(self, wallet_name: str, error: str | None = None):
        super().__init__()
        self.wallet_name = wallet_name
        self.error = error

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(f"Unlock {self.wallet_name}")
            yield Static("Unlocking enables signing for this TUI session until you lock, switch wallets, or quit.")
            yield Input(placeholder="Wallet password", password=True, id="unlock-password")
            yield Static(self.error or "", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Unlock", id="unlock", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id == "unlock":
            password = self.query_one("#unlock-password", Input).value
            if not password:
                self.query_one("#form-error", Static).update("Wallet password is required.")
                return
            self.dismiss(password)


class WatchWalletDialog(ModalScreen[WatchWalletRequest | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    WatchWalletDialog { align: center middle; }
    WatchWalletDialog > #dialog { width: 82; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    WatchWalletDialog Input, WatchWalletDialog Select { margin-top: 1; }
    WatchWalletDialog #form-error { color: $error; margin-top: 1; }
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
            yield Select(NETWORK_OPTIONS, value=self.network, allow_blank=False, id="watch-network")
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


class CreateWalletDialog(ModalScreen[CreateWalletRequest | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    CreateWalletDialog { align: center middle; }
    CreateWalletDialog > #dialog { width: 86; height: auto; max-height: 95%; padding: 1 2; border: round $accent; background: $surface; }
    CreateWalletDialog Input, CreateWalletDialog Select { margin-top: 1; }
    CreateWalletDialog #form-error { color: $error; margin-top: 1; }
    CreateWalletDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    CreateWalletDialog Button { margin-left: 1; }
    """

    def __init__(self, network: str):
        super().__init__()
        self.network = network

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Create wallet")
            yield Input(placeholder="Wallet name", id="name")
            yield Select(NETWORK_OPTIONS, value=self.network, allow_blank=False, id="network")
            yield Select(LANGUAGE_OPTIONS, value="english", allow_blank=False, id="language")
            yield Select(
                [("12 words / 128-bit", "128"), ("24 words / 256-bit", "256")],
                value="256",
                allow_blank=False,
                id="strength",
            )
            yield Input(value="0", placeholder="Account index", id="index")
            yield Input(placeholder="BIP39 passphrase (optional)", password=True, id="mnemonic-passphrase")
            yield Input(placeholder="New wallet password", password=True, id="password")
            yield Input(placeholder="Confirm wallet password", password=True, id="password-confirm")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Create", id="create", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "create":
            return
        name = self.query_one("#name", Input).value.strip()
        password = self.query_one("#password", Input).value
        confirmation = self.query_one("#password-confirm", Input).value
        index = _parse_index(self.query_one("#index", Input).value, self.query_one("#form-error", Static))
        if index is None:
            return
        if not name or not _valid_new_password(password, confirmation, self.query_one("#form-error", Static)):
            if not name:
                self.query_one("#form-error", Static).update("Wallet name is required.")
            return
        self.dismiss(
            CreateWalletRequest(
                name=name,
                network=str(self.query_one("#network", Select).value),
                language=str(self.query_one("#language", Select).value),
                strength=int(str(self.query_one("#strength", Select).value)),
                index=index,
                mnemonic_passphrase=self.query_one("#mnemonic-passphrase", Input).value,
                password=password,
            )
        )


class ImportSecretDialog(ModalScreen[ImportSecretRequest | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = CreateWalletDialog.CSS.replace("CreateWalletDialog", "ImportSecretDialog")

    def __init__(self, network: str):
        super().__init__()
        self.network = network

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Import Stellar secret")
            yield Input(placeholder="Wallet name", id="name")
            yield Select(NETWORK_OPTIONS, value=self.network, allow_blank=False, id="network")
            yield Input(placeholder="Stellar secret S...", password=True, id="secret")
            yield Input(placeholder="New wallet password", password=True, id="password")
            yield Input(placeholder="Confirm wallet password", password=True, id="password-confirm")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Import", id="import", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "import":
            return
        error = self.query_one("#form-error", Static)
        name = self.query_one("#name", Input).value.strip()
        secret = self.query_one("#secret", Input).value.strip()
        password = self.query_one("#password", Input).value
        confirmation = self.query_one("#password-confirm", Input).value
        if not name or not secret:
            error.update("Wallet name and Stellar secret are required.")
            return
        if not _valid_new_password(password, confirmation, error):
            return
        self.dismiss(
            ImportSecretRequest(
                name=name,
                network=str(self.query_one("#network", Select).value),
                secret=secret,
                password=password,
            )
        )


class ImportMnemonicDialog(ModalScreen[ImportMnemonicRequest | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = CreateWalletDialog.CSS.replace("CreateWalletDialog", "ImportMnemonicDialog")

    def __init__(self, network: str):
        super().__init__()
        self.network = network

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Import mnemonic")
            yield Input(placeholder="Wallet name", id="name")
            yield Select(NETWORK_OPTIONS, value=self.network, allow_blank=False, id="network")
            yield Input(placeholder="Mnemonic phrase", password=True, id="mnemonic")
            yield Select(
                [("Auto-detect language", "auto"), *LANGUAGE_OPTIONS],
                value="auto",
                allow_blank=False,
                id="language",
            )
            yield Input(value="0", placeholder="Account index", id="index")
            yield Input(placeholder="BIP39 passphrase (optional)", password=True, id="mnemonic-passphrase")
            yield Input(placeholder="New wallet password", password=True, id="password")
            yield Input(placeholder="Confirm wallet password", password=True, id="password-confirm")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Import", id="import", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "import":
            return
        error = self.query_one("#form-error", Static)
        name = self.query_one("#name", Input).value.strip()
        mnemonic = self.query_one("#mnemonic", Input).value.strip()
        password = self.query_one("#password", Input).value
        confirmation = self.query_one("#password-confirm", Input).value
        index = _parse_index(self.query_one("#index", Input).value, error)
        if index is None:
            return
        if not name or not mnemonic:
            error.update("Wallet name and mnemonic are required.")
            return
        if not _valid_new_password(password, confirmation, error):
            return
        language = str(self.query_one("#language", Select).value)
        self.dismiss(
            ImportMnemonicRequest(
                name=name,
                network=str(self.query_one("#network", Select).value),
                mnemonic=mnemonic,
                language=None if language == "auto" else language,
                index=index,
                mnemonic_passphrase=self.query_one("#mnemonic-passphrase", Input).value,
                password=password,
            )
        )


class MnemonicBackupDialog(ModalScreen[None]):
    BINDINGS = [("escape", "close", "Close"), ("enter", "close", "Done")]

    CSS = """
    MnemonicBackupDialog { align: center middle; }
    MnemonicBackupDialog > #dialog { width: 88; height: auto; padding: 1 2; border: round $warning; background: $surface; }
    MnemonicBackupDialog #mnemonic { margin: 1 0; text-style: bold; }
    MnemonicBackupDialog #actions { height: auto; align-horizontal: right; }
    """

    def __init__(self, wallet_name: str, mnemonic: str):
        super().__init__()
        self.wallet_name = wallet_name
        self.mnemonic = mnemonic

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(f"Back up {self.wallet_name}")
            yield Static("These words are shown once and are not stored in plaintext.")
            yield Static(self.mnemonic, id="mnemonic")
            with Horizontal(id="actions"):
                yield Button("I have backed it up", id="close", variant="warning")

    def action_close(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "close":
            self.action_close()


class ConfirmDialog(ModalScreen[bool]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    ConfirmDialog { align: center middle; }
    ConfirmDialog > #dialog { width: 70; height: auto; padding: 1 2; border: round $warning; background: $surface; }
    ConfirmDialog #message { margin: 1 0; }
    ConfirmDialog #actions { height: auto; align-horizontal: right; }
    ConfirmDialog Button { margin-left: 1; }
    """

    def __init__(self, title: str, message: str, confirm_label: str = "Confirm"):
        super().__init__()
        self.dialog_title = title
        self.message = message
        self.confirm_label = confirm_label

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(self.dialog_title)
            yield Static(self.message, id="message")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button(self.confirm_label, id="confirm", variant="error")

    def action_cancel(self) -> None:
        self.dismiss(False)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(event.button.id == "confirm")


class NoticeDialog(ModalScreen[None]):
    BINDINGS = [("escape", "close", "Close"), ("enter", "close", "Close")]

    CSS = """
    NoticeDialog { align: center middle; }
    NoticeDialog > #dialog { width: 68; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    NoticeDialog #message { margin: 1 0; }
    NoticeDialog #actions { height: auto; align-horizontal: right; }
    """

    def __init__(self, title: str, message: str):
        super().__init__()
        self.dialog_title = title
        self.message = message

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(self.dialog_title)
            yield Static(self.message, id="message")
            with Horizontal(id="actions"):
                yield Button("Close", id="close", variant="primary")

    def action_close(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "close":
            self.action_close()


class ErrorDialog(ModalScreen[None]):
    BINDINGS = [("escape", "close", "Close"), ("enter", "close", "Close")]

    CSS = """
    ErrorDialog { align: center middle; }
    ErrorDialog > #dialog { width: 82; height: auto; max-height: 90%; padding: 1 2; border: round $error; background: $surface; }
    ErrorDialog #message { margin: 1 0; }
    ErrorDialog #details { color: $text-muted; margin-bottom: 1; }
    ErrorDialog #actions { height: auto; align-horizontal: right; }
    """

    def __init__(self, message: str, details: str | None = None):
        super().__init__()
        self.message = message
        self.details = details

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Operation failed")
            yield Static(self.message, id="message")
            if self.details:
                yield Static(f"DEV {self.details}", id="details")
            with Horizontal(id="actions"):
                yield Button("Close", id="close", variant="error")

    def action_close(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "close":
            self.action_close()


class SendDialog(ModalScreen[SendRequest | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    SendDialog { align: center middle; }
    SendDialog > #dialog { width: 82; height: auto; max-height: 90%; padding: 1 2; border: round $accent; background: $surface; }
    SendDialog Input { margin-top: 1; }
    SendDialog #form-error { color: $error; margin-top: 1; }
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
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Review", id="review", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "review":
            return
        amount = self.query_one("#amount", Input).value.strip()
        asset = self.query_one("#asset", Input).value.strip()
        destination = self.query_one("#destination", Input).value.strip()
        memo = self.query_one("#memo", Input).value.strip()
        if not amount or not asset or not destination:
            self.query_one("#form-error", Static).update("Amount, asset, and destination are required.")
            return
        self.dismiss(SendRequest(amount, asset, destination, memo or None))


class ReviewDialog(ModalScreen[bool]):
    BINDINGS = [("escape", "cancel", "Cancel")]

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

    def action_cancel(self) -> None:
        self.dismiss(False)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(event.button.id == "confirm")


def _parse_index(value: str, error_widget: Static) -> int | None:
    try:
        index = int(value.strip() or "0")
    except ValueError:
        error_widget.update("Account index must be an integer.")
        return None
    if index < 0:
        error_widget.update("Account index cannot be negative.")
        return None
    return index


def _valid_new_password(password: str, confirmation: str, error_widget: Static) -> bool:
    if not password:
        error_widget.update("Wallet password is required.")
        return False
    if password != confirmation:
        error_widget.update("Wallet passwords do not match.")
        return False
    return True

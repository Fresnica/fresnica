"""TUI-only system-unlock enrollment UI."""

from textual.app import ComposeResult
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, Input, Label, Static


class SystemUnlockEnrollmentDialog(ModalScreen[str | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    SystemUnlockEnrollmentDialog { align: center middle; }
    SystemUnlockEnrollmentDialog > #dialog {
        width: 68; height: auto; padding: 1 2;
        border: round $accent; background: $surface;
    }
    SystemUnlockEnrollmentDialog Input { margin-top: 1; }
    SystemUnlockEnrollmentDialog #form-error { color: $error; margin-top: 1; }
    SystemUnlockEnrollmentDialog #actions {
        height: auto; margin-top: 1; align-horizontal: right;
    }
    SystemUnlockEnrollmentDialog Button { margin-left: 1; }
    """

    def __init__(self, wallet_name: str, backend_label: str, error: str | None = None):
        super().__init__()
        self.wallet_name = wallet_name
        self.backend_label = backend_label
        self.error = error

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(f"Enable system unlock · {self.wallet_name}")
            yield Static(
                f"Enter the Fresnica app passcode once. Fresnica will derive and verify this "
                f"wallet's 32-byte unlock key, then hand only that key to {self.backend_label}. "
                "The mnemonic/private key is not stored by the system-auth backend."
            )
            yield Input(placeholder="Fresnica app passcode", password=True, id="passcode")
            yield Static(self.error or "", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Enable", id="enable", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "enable":
            return
        passcode = self.query_one("#passcode", Input).value
        if not passcode:
            self.query_one("#form-error", Static).update("App passcode is required.")
            return
        self.dismiss(passcode)

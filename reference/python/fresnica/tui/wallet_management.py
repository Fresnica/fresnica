"""Wallet-management UI organized around selection and context."""

from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Label, Static

from ..manager import WalletState
from .screens import WalletAction


class WalletManagerDialog(ModalScreen[WalletAction | None]):
    BINDINGS = [
        Binding("escape", "cancel", "Close", show=False),
        Binding("a", "add", "Add wallet", show=False),
        Binding("l", "toggle_lock", "Lock / Unlock", show=False),
        Binding("f", "fund", "Fund testnet", show=False),
        Binding("d", "delete", "Remove wallet", show=False),
    ]

    CSS = """
    WalletManagerDialog { align: center middle; }
    WalletManagerDialog > #dialog {
        width: 92%; max-width: 112; min-width: 68;
        height: auto; max-height: 92%;
        padding: 1 2; border: round $accent; background: $surface;
    }
    WalletManagerDialog #wallet-list { height: 12; margin: 1 0; }
    WalletManagerDialog #wallet-detail { min-height: 4; margin-bottom: 1; }
    WalletManagerDialog .section-label { text-style: bold; color: $text-muted; margin-top: 1; }
    WalletManagerDialog .action-row { height: auto; margin-top: 1; }
    WalletManagerDialog Button { margin-right: 1; }
    WalletManagerDialog #library-actions { align-horizontal: left; }
    WalletManagerDialog #context-actions { align-horizontal: left; }
    WalletManagerDialog #danger-actions { align-horizontal: left; }
    WalletManagerDialog #close-actions { align-horizontal: right; }
    """

    def __init__(self, records, current_name: str | None, states: dict[str, WalletState]):
        super().__init__()
        self.records = list(records)
        self.current_name = current_name
        self.states = states

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Wallets")
            yield Static("Choose a wallet with Enter. Moving through the list only previews it.", id="wallet-hint")
            yield DataTable(id="wallet-list")
            yield Static("", id="wallet-detail")

            yield Label("Wallet actions", classes="section-label")
            with Horizontal(id="context-actions", classes="action-row"):
                yield Button("Unlock", id="lock")
                yield Button("Fund on testnet", id="fund")

            yield Label("Wallet library", classes="section-label")
            with Horizontal(id="library-actions", classes="action-row"):
                yield Button("Add wallet", id="add", variant="primary")

            yield Label("Danger zone", classes="section-label", id="danger-label")
            with Horizontal(id="danger-actions", classes="action-row"):
                yield Button("Remove wallet", id="delete", variant="error")

            with Horizontal(id="close-actions", classes="action-row"):
                yield Button("Close", id="cancel")

    def on_mount(self) -> None:
        table = self.query_one("#wallet-list", DataTable)
        table.add_columns("Name", "Network", "Access", "Status")
        table.cursor_type = "row"
        current_row = 0
        for index, record in enumerate(self.records):
            state = self.states[record.name]
            if record.name == self.current_name:
                current_row = index
            table.add_row(
                ("● " if record.name == self.current_name else "  ") + record.name,
                record.network.upper(),
                _access_label(record, state),
                _state_label(state),
                key=record.name,
            )
        table.move_cursor(row=current_row)
        self._refresh_selection()

    def on_data_table_row_highlighted(self, event: DataTable.RowHighlighted) -> None:
        if event.data_table.id == "wallet-list":
            self._refresh_selection()

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        if event.data_table.id == "wallet-list":
            self.dismiss(WalletAction("use", self._selected_record().name))

    def _selected_record(self):
        table = self.query_one("#wallet-list", DataTable)
        index = max(0, min(table.cursor_row, len(self.records) - 1))
        return self.records[index]

    def _refresh_selection(self) -> None:
        record = self._selected_record()
        state = self.states[record.name]
        active = "Current wallet" if record.name == self.current_name else "Press Enter to use this wallet"
        self.query_one("#wallet-detail", Static).update(
            f"{record.name}\n{record.address}\n{record.network.upper()} · {_detail_access(record, state)}\n{active}"
        )

        lock = self.query_one("#lock", Button)
        lock.display = state is not WalletState.WATCH_ONLY
        if state is WalletState.UNLOCKED:
            lock.label = "Lock"
        else:
            lock.label = "Unlock"

        fund = self.query_one("#fund", Button)
        fund.display = record.network == "testnet"

    def action_cancel(self) -> None:
        self.dismiss(None)

    def action_add(self) -> None:
        self.dismiss(WalletAction("add"))

    def action_toggle_lock(self) -> None:
        record = self._selected_record()
        state = self.states[record.name]
        if state is WalletState.WATCH_ONLY:
            return
        self.dismiss(
            WalletAction("lock" if state is WalletState.UNLOCKED else "unlock", record.name)
        )

    def action_fund(self) -> None:
        record = self._selected_record()
        if record.network == "testnet":
            self.dismiss(WalletAction("fund", record.name))

    def action_delete(self) -> None:
        self.dismiss(WalletAction("delete", self._selected_record().name))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        actions = {
            "cancel": self.action_cancel,
            "add": self.action_add,
            "lock": self.action_toggle_lock,
            "fund": self.action_fund,
            "delete": self.action_delete,
        }
        action = actions.get(event.button.id)
        if action is not None:
            action()


def _access_label(record, state: WalletState) -> str:
    if state is WalletState.WATCH_ONLY:
        return "Watch-only"
    return "Signing"


def _state_label(state: WalletState) -> str:
    if state is WalletState.WATCH_ONLY:
        return "—"
    return "Unlocked" if state is WalletState.UNLOCKED else "Locked"


def _detail_access(record, state: WalletState) -> str:
    if state is WalletState.WATCH_ONLY:
        return "Watch-only · no signing key"
    kind = "Mnemonic wallet" if record.wallet_type == "mnemonic" else "Secret-key wallet"
    status = "Unlocked" if state is WalletState.UNLOCKED else "Locked"
    return f"{kind} · {status}"

"""Wallet-management UI organized around selection and context."""

from collections.abc import Callable

from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Label, Static

from ..manager import WalletState
from .screens import WalletAction


class WalletManagerDialog(ModalScreen[WalletAction | None]):
    BINDINGS = [
        Binding("escape", "close", "Close", show=False),
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
    WalletManagerDialog #context-actions { align-horizontal: left; }
    WalletManagerDialog #secondary-sections { height: auto; }
    WalletManagerDialog .secondary-section { width: 1fr; height: auto; }
    WalletManagerDialog #danger-section { padding-left: 2; }
    WalletManagerDialog #library-actions { align-horizontal: left; }
    WalletManagerDialog #danger-actions { align-horizontal: left; }
    WalletManagerDialog #close-actions { align-horizontal: right; }
    """

    def __init__(
        self,
        records,
        current_name: str | None,
        states: dict[str, WalletState],
        account_exists: dict[str, bool | None] | None = None,
        on_select: Callable[[str], dict[str, WalletState] | None] | None = None,
    ):
        super().__init__()
        self.records = list(records)
        self.initial_name = current_name
        self.current_name = current_name
        self.states = dict(states)
        self.account_exists = dict(account_exists or {})
        self.on_select = on_select

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Wallets")
            yield Static(
                "Move to preview. Enter chooses the current wallet; Dashboard updates after Close.",
                id="wallet-hint",
            )
            yield DataTable(id="wallet-list")
            yield Static("", id="wallet-detail")

            yield Label("Wallet actions", classes="section-label")
            with Horizontal(id="context-actions", classes="action-row"):
                yield Button("Unlock", id="lock")
                yield Button("Fund on testnet", id="fund")

            with Horizontal(id="secondary-sections"):
                with Vertical(id="library-section", classes="secondary-section"):
                    yield Label("Wallet library", classes="section-label")
                    with Horizontal(id="library-actions", classes="action-row"):
                        yield Button("Add wallet", id="add", variant="primary")

                with Vertical(id="danger-section", classes="secondary-section"):
                    yield Label("Danger zone", classes="section-label")
                    with Horizontal(id="danger-actions", classes="action-row"):
                        yield Button("Remove wallet", id="delete", variant="error")

            with Horizontal(id="close-actions", classes="action-row"):
                yield Button("Close", id="close")

    def on_mount(self) -> None:
        table = self.query_one("#wallet-list", DataTable)
        table.add_columns("Name", "Network", "Access", "Status")
        table.cursor_type = "row"
        self._populate_rows(self.current_name)
        self._refresh_selection()

    def on_data_table_row_highlighted(self, event: DataTable.RowHighlighted) -> None:
        if event.data_table.id == "wallet-list":
            self._refresh_selection()

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        if event.data_table.id != "wallet-list":
            return
        record = self._selected_record()
        if record.name != self.current_name:
            if self.on_select is not None:
                states = self.on_select(record.name)
                if states:
                    self.states.update(states)
            self.current_name = record.name
            self._populate_rows(record.name)
        self._refresh_selection()

    def _populate_rows(self, selected_name: str | None) -> None:
        table = self.query_one("#wallet-list", DataTable)
        selected_row = 0
        table.clear(columns=False)
        for index, record in enumerate(self.records):
            state = self.states[record.name]
            if record.name == selected_name:
                selected_row = index
            table.add_row(
                ("● " if record.name == self.current_name else "  ") + record.name,
                record.network.upper(),
                _access_label(record, state),
                _state_label(state),
                key=record.name,
            )
        table.move_cursor(row=selected_row)

    def _selected_record(self):
        table = self.query_one("#wallet-list", DataTable)
        index = max(0, min(table.cursor_row, len(self.records) - 1))
        return self.records[index]

    def _refresh_selection(self) -> None:
        record = self._selected_record()
        state = self.states[record.name]
        active = (
            "Current wallet"
            if record.name == self.current_name
            else "Press Enter to make this wallet current"
        )
        lines = [
            record.name,
            record.address,
            f"{record.network.upper()} · {_detail_access(record, state)}",
            active,
        ]
        if record.network == "testnet":
            exists = self.account_exists.get(record.name)
            if exists is True:
                lines.append("Testnet account exists")
            elif exists is False:
                lines.append("Testnet account is not funded")
            else:
                lines.append("Testnet account status not cached")
        self.query_one("#wallet-detail", Static).update("\n".join(lines))

        lock = self.query_one("#lock", Button)
        lock.display = state is not WalletState.WATCH_ONLY
        lock.label = "Lock" if state is WalletState.UNLOCKED else "Unlock"

        fund = self.query_one("#fund", Button)
        fund.display = (
            record.network == "testnet"
            and self.account_exists.get(record.name) is not True
        )

    def action_close(self) -> None:
        if self.current_name != self.initial_name and self.current_name is not None:
            self.dismiss(WalletAction("use", self.current_name))
        else:
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
        if (
            record.network == "testnet"
            and self.account_exists.get(record.name) is not True
        ):
            self.dismiss(WalletAction("fund", record.name))

    def action_delete(self) -> None:
        self.dismiss(WalletAction("delete", self._selected_record().name))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        actions = {
            "close": self.action_close,
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

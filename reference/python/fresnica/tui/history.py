"""Full account activity screen backed by the local operation cache."""

from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.screen import ModalScreen
from textual.widgets import DataTable, Footer, Static

from ..errors import FresnicaError
from ..presentation import format_timestamp


class HistoryScreen(ModalScreen[None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("r", "refresh", "Refresh"),
        Binding("m", "more", "Older"),
    ]

    CSS = """
    HistoryScreen {
        layout: vertical;
        background: $surface;
        padding: 1 2;
    }
    #history-title { height: 1; text-style: bold; }
    #history-status { height: 1; color: $text-muted; margin-bottom: 1; }
    #history-table { height: 1fr; }
    """

    def __init__(self, history_service, wallet, wallet_name: str, initial_limit: int = 200):
        super().__init__()
        self.history_service = history_service
        self.wallet = wallet
        self.wallet_name = wallet_name
        self.limit = initial_limit

    def compose(self) -> ComposeResult:
        yield Static(f"Activity · {self.wallet_name}", id="history-title")
        yield Static("Loading local activity...", id="history-status")
        yield DataTable(id="history-table")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#history-table", DataTable)
        table.add_columns("Time", "Activity")
        table.cursor_type = "row"
        self._load_cached()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_refresh(self) -> None:
        self.query_one("#history-status", Static).update("Refreshing recent activity...")
        self._refresh_history()

    def action_more(self) -> None:
        self.query_one("#history-status", Static).update("Loading older activity...")
        self._load_more()

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _load_cached(self) -> None:
        try:
            views = self.history_service.get_activity_views(
                self.wallet,
                limit=self.limit,
                refresh=False,
            )
            self.app.call_from_thread(self._apply, views, "Local activity", None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply, [], "", exc)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_history(self) -> None:
        try:
            views = self.history_service.get_activity_views(
                self.wallet,
                limit=self.limit,
                refresh=True,
            )
            self.app.call_from_thread(self._apply, views, "Activity updated", None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply, [], "", exc)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _load_more(self) -> None:
        try:
            added = self.history_service.load_older(self.wallet, limit=200)
            self.limit += 200
            views = self.history_service.get_activity_views(
                self.wallet,
                limit=self.limit,
                refresh=False,
            )
            message = f"Loaded {added} older operations" if added else "No older operations returned"
            self.app.call_from_thread(self._apply, views, message, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply, [], "", exc)

    def _apply(self, views, message: str, error) -> None:
        table = self.query_one("#history-table", DataTable)
        table.clear()
        for item in views:
            table.add_row(format_timestamp(item.created_at), item.summary)
        status = self.query_one("#history-status", Static)
        if error is not None:
            details = getattr(error, "details", None)
            text = f"ERROR {error}"
            if details:
                text += f" · DEV {details}"
            status.update(text)
            return
        status.update(f"{message} · {len(views)} cached activities shown")

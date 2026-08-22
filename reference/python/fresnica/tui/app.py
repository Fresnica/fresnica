"""Textual dashboard for interactive Fresnica mode."""

from rich.text import Text
from textual import work
from textual.app import App, ComposeResult
from textual.widgets import DataTable, Footer, Header, Static

from ..errors import FresnicaError


class FresnicaApp(App[None]):
    TITLE = "Fresnica"
    SUB_TITLE = "Stellar Wallet"
    BINDINGS = [
        ("r", "refresh", "Refresh"),
        ("q", "quit", "Quit"),
    ]

    CSS = """
    #wallet { padding: 1 2; }
    #status { padding: 0 2 1 2; }
    #balances { height: 1fr; }
    """

    def __init__(self, runtime):
        super().__init__()
        self.runtime = runtime

    def compose(self) -> ComposeResult:
        yield Header()
        yield Static("Loading wallet...", id="wallet")
        yield Static("", id="status")
        yield DataTable(id="balances")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#balances", DataTable)
        table.add_columns("Asset", "Balance", "Liabilities", "Available")
        self.refresh_wallet()

    def action_refresh(self) -> None:
        self.refresh_wallet()

    @work(exclusive=True, thread=True, exit_on_error=False)
    def refresh_wallet(self) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            services = self.runtime.services_for(session.record.network)
            views = services.balance_service.get_views(session.wallet)
            self.call_from_thread(self._apply_wallet, session.record, views, None)
        except FresnicaError as exc:
            self.call_from_thread(self._apply_wallet, None, [], str(exc))

    def _apply_wallet(self, record, views, error: str | None) -> None:
        wallet_widget = self.query_one("#wallet", Static)
        status_widget = self.query_one("#status", Static)
        table = self.query_one("#balances", DataTable)
        table.clear()

        if record is None:
            wallet_widget.update("No wallet selected")
            status_widget.update(Text(error or "", style="red"))
            return

        wallet_widget.update(
            f"{record.name}  {record.address}  [{record.network}]  {record.wallet_type}"
        )
        status_widget.update(Text(error or "Ready", style="red" if error else "green"))
        for item in views:
            table.add_row(
                item.asset.display,
                str(item.balance),
                str(item.selling_liabilities),
                str(item.available),
            )


def run_tui(runtime=None):
    if runtime is None:
        from ..runtime import Runtime

        runtime = Runtime()
    FresnicaApp(runtime).run()

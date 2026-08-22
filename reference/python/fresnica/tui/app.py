"""Textual wallet shell for interactive Fresnica mode."""

from rich.text import Text
from textual import work
from textual.app import App, ComposeResult
from textual.widgets import DataTable, Footer, Header, Label, Static

from ..errors import FresnicaError, WatchOnlyError
from .screens import ReviewDialog, SendDialog, WalletPicker


class FresnicaApp(App[None]):
    TITLE = "Fresnica"
    SUB_TITLE = "Stellar Wallet"
    BINDINGS = [
        ("r", "refresh", "Refresh"),
        ("w", "switch_wallet", "Wallet"),
        ("h", "refresh_history", "History"),
        ("s", "send", "Send"),
        ("q", "quit", "Quit"),
    ]

    CSS = """
    #wallet { padding: 1 2 0 2; text-style: bold; }
    #status { padding: 0 2 1 2; height: auto; }
    .section-title { padding: 0 2; text-style: bold; }
    #balances { height: 2fr; min-height: 8; }
    #history { height: 2fr; min-height: 8; }
    """

    def __init__(self, runtime):
        super().__init__()
        self.runtime = runtime
        self._pending_send = None

    def compose(self) -> ComposeResult:
        yield Header()
        yield Static("Loading wallet...", id="wallet")
        yield Static("", id="status")
        yield Label("Balances", classes="section-title")
        yield DataTable(id="balances")
        yield Label("Recent activity", classes="section-title")
        yield DataTable(id="history")
        yield Footer()

    def on_mount(self) -> None:
        balances = self.query_one("#balances", DataTable)
        balances.add_columns("Asset", "Balance", "Liabilities", "Available")
        balances.cursor_type = "row"
        history = self.query_one("#history", DataTable)
        history.add_columns("Time", "Type", "Summary")
        history.cursor_type = "row"
        self.refresh_wallet()

    def on_unmount(self) -> None:
        self.runtime.wallet_manager.lock()

    def action_refresh(self) -> None:
        self.refresh_wallet()

    def action_refresh_history(self) -> None:
        self.refresh_wallet()

    def action_switch_wallet(self) -> None:
        manager = self.runtime.wallet_manager
        records = manager.list_wallets()
        if not records:
            self._set_status("No wallets available", error=True)
            return
        try:
            current_name = manager.get_record().name
        except FresnicaError:
            current_name = records[0].name
        self.push_screen(WalletPicker(records, current_name), self._on_wallet_selected)

    def _on_wallet_selected(self, name: str | None) -> None:
        if not name:
            return
        self.runtime.wallet_manager.set_default(name)
        self.runtime.wallet_manager.lock()
        self._set_status(f"Selected wallet {name}")
        self.refresh_wallet()

    def action_send(self) -> None:
        manager = self.runtime.wallet_manager
        try:
            record = manager.get_record()
            if record.watch_only:
                raise WatchOnlyError(f'Wallet "{record.name}" is watch-only')
        except FresnicaError as exc:
            self._set_error(exc)
            return
        self.push_screen(SendDialog(record.name), self._on_send_request)

    def _on_send_request(self, request) -> None:
        if request is None:
            return
        self._set_status("Preparing transaction...")
        self.prepare_send(request)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def refresh_wallet(self) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            services = self.runtime.services_for(session.record.network)
            balances = services.balance_service.get_views(session.wallet)
            history = services.history_service.get_views(session.wallet, limit=20)
            self.call_from_thread(
                self._apply_wallet,
                session.record,
                balances,
                history,
                None,
            )
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._apply_wallet, None, [], [], exc)

    @work(thread=True, exit_on_error=False)
    def prepare_send(self, request) -> None:
        manager = self.runtime.wallet_manager
        try:
            record = manager.get_record()
            session = manager.unlock(record.name, request.password)
            services = self.runtime.services_for(record.network)
            prepared = services.transfer_service.prepare(
                wallet_name=record.name,
                wallet=session.wallet,
                destination=request.destination,
                asset=request.asset,
                amount=request.amount,
                memo=request.memo,
            )
            self.call_from_thread(
                self._show_review,
                session.wallet,
                services,
                prepared,
                record.network,
            )
        except (FresnicaError, ValueError) as exc:
            manager.lock()
            self.call_from_thread(self._set_error, exc)

    def _show_review(self, wallet, services, prepared, network: str) -> None:
        self._pending_send = (wallet, services, prepared, network)
        self._set_status("Transaction ready for review")
        self.push_screen(ReviewDialog(prepared.review), self._on_review)

    def _on_review(self, confirmed: bool) -> None:
        if not confirmed:
            self.runtime.wallet_manager.lock()
            self._pending_send = None
            self._set_status("Transaction cancelled")
            return
        self._set_status("Submitting transaction...")
        self.submit_pending()

    @work(thread=True, exit_on_error=False)
    def submit_pending(self) -> None:
        pending = self._pending_send
        if pending is None:
            return
        wallet, services, prepared, network = pending
        try:
            services.transfer_service.sign(wallet, prepared)
            result = services.transfer_service.submit(prepared)
            self.call_from_thread(self._finish_send, result, network, None)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._finish_send, None, network, exc)
        finally:
            self.runtime.wallet_manager.lock()
            self._pending_send = None

    def _finish_send(self, result, network: str, error) -> None:
        if error is not None:
            self._set_error(error)
            return
        self._set_status(
            f"Submitted on {network}: {result.hash}"
            + (f"  ledger {result.ledger}" if result.ledger is not None else "")
        )
        self.refresh_wallet()

    def _apply_wallet(self, record, balances, history, error) -> None:
        wallet_widget = self.query_one("#wallet", Static)
        balance_table = self.query_one("#balances", DataTable)
        history_table = self.query_one("#history", DataTable)
        balance_table.clear()
        history_table.clear()

        if record is None:
            wallet_widget.update("No wallet selected")
            if error is not None:
                self._set_error(error)
            return

        wallet_widget.update(
            f"{record.name}  {record.address}  [{record.network}]  {record.wallet_type}"
        )
        self._set_status("Ready")
        for item in balances:
            balance_table.add_row(
                item.asset.display,
                str(item.balance),
                str(item.selling_liabilities),
                str(item.available),
            )
        for item in history:
            history_table.add_row(
                item.created_at or "",
                item.operation_type,
                item.summary,
            )

    def _set_error(self, exc) -> None:
        self._set_status(
            str(exc),
            error=True,
            details=getattr(exc, "details", None),
        )

    def _set_status(self, message: str, error: bool = False, details: str | None = None) -> None:
        status = Text(message, style="red" if error else "green")
        if details:
            status.append(f"\nDEV {details}", style="dim")
        self.query_one("#status", Static).update(status)


def run_tui(runtime=None):
    if runtime is None:
        from ..runtime import Runtime

        runtime = Runtime()
    FresnicaApp(runtime).run()

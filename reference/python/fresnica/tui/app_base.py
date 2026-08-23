"""State-driven Textual wallet dashboard for interactive Fresnica mode."""

from datetime import datetime

from rich.text import Text
from textual import events, work
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.widgets import DataTable, Footer, Header, Label, Static

from ..contacts import resolve_destination
from ..errors import FresnicaError, NetworkError, WalletLockedError, WalletNotFoundError
from ..manager import WalletState
from ..presentation import (
    asset_label,
    asset_source,
    format_amount,
    format_timestamp,
    short_pool_id,
)
from .history import HistoryScreen
from .screens import (
    AddWalletDialog,
    ConfirmDialog,
    CreateWalletDialog,
    ErrorDialog,
    ImportMnemonicDialog,
    ImportSecretDialog,
    MnemonicBackupDialog,
    NoticeDialog,
    ReviewDialog,
    SendDialog,
    UnlockDialog,
    WalletAction,
    WalletManagerDialog,
    WatchWalletDialog,
)


WIDE_DASHBOARD_COLUMNS = 120


class FresnicaApp(App[None]):
    TITLE = "Fresnica"
    SUB_TITLE = "Stellar Wallet"
    BINDINGS = [
        Binding("r", "refresh", "Refresh"),
        Binding("w", "manage_wallets", "Wallets"),
        Binding("h", "history", "History"),
        Binding("z", "toggle_zero", "Zero assets"),
        Binding("s", "send", "Send", show=False),
        Binding("l", "toggle_lock", "Lock / Unlock", show=False),
        Binding("q", "quit", "Quit"),
    ]

    CSS = """
    #wallet { padding: 1 2 0 2; height: auto; text-style: bold; }
    #wallet-actions { padding: 0 2; height: auto; color: $text-muted; }
    #status { padding: 0 2; height: auto; }
    #sync-status { padding: 0 2 1 2; height: 2; text-align: right; color: $text-muted; }
    .section-title { padding: 0 1; height: 1; text-style: bold; }
    #dashboard { height: 1fr; layout: vertical; }
    #portfolio-pane, #activity-pane { width: 1fr; height: 1fr; padding: 0 1; }
    #dashboard.wide { layout: horizontal; }
    #dashboard.wide #portfolio-pane { width: 3fr; height: 1fr; }
    #dashboard.wide #activity-pane { width: 2fr; height: 1fr; }
    #balances { height: 2fr; min-height: 7; }
    #liquidity { height: 1fr; min-height: 5; }
    #history { height: 1fr; min-height: 8; }
    """

    def __init__(self, runtime):
        super().__init__()
        self.runtime = runtime
        self._pending_send = None
        self._show_zero_balances = False
        self._last_record = None
        self._last_balances = []
        self._last_positions = []
        self._last_history = []

    def compose(self) -> ComposeResult:
        yield Header()
        yield Static("Loading wallet...", id="wallet")
        yield Static("", id="wallet-actions")
        yield Static("", id="status")
        yield Static("", id="sync-status")
        with Horizontal(id="dashboard"):
            with Vertical(id="portfolio-pane"):
                yield Label("Assets", id="assets-title", classes="section-title")
                yield DataTable(id="balances")
                yield Label("Liquidity positions", id="liquidity-title", classes="section-title")
                yield DataTable(id="liquidity")
            with Vertical(id="activity-pane"):
                yield Label("Recent activity", classes="section-title")
                yield DataTable(id="history")
        yield Footer()

    def on_mount(self) -> None:
        balances = self.query_one("#balances", DataTable)
        balances.add_columns("Asset", "Issuer / source", "Balance", "Available", "In offers")
        balances.cursor_type = "row"
        liquidity = self.query_one("#liquidity", DataTable)
        liquidity.add_columns("Pool", "Shares", "Position")
        liquidity.cursor_type = "row"
        history = self.query_one("#history", DataTable)
        history.add_columns("Time", "Activity")
        history.cursor_type = "row"
        self._apply_layout(self.size.width)
        self.refresh_wallet()

    def on_resize(self, event: events.Resize) -> None:
        self._apply_layout(event.size.width)

    def on_unmount(self) -> None:
        self.runtime.wallet_manager.lock()

    def _apply_layout(self, width: int) -> None:
        dashboard = self.query_one("#dashboard", Horizontal)
        if width >= WIDE_DASHBOARD_COLUMNS:
            dashboard.add_class("wide")
        else:
            dashboard.remove_class("wide")

    def action_refresh(self) -> None:
        self.refresh_wallet()

    def action_history(self) -> None:
        try:
            session = self.runtime.wallet_manager.view()
        except WalletNotFoundError:
            self._show_notice("No wallet", "Add or import a wallet before viewing history.")
            return
        services = self.runtime.services_for(session.record.network)
        self.push_screen(
            HistoryScreen(
                services.history_service,
                session.wallet,
                session.record.name,
            )
        )

    def action_toggle_zero(self) -> None:
        self._show_zero_balances = not self._show_zero_balances
        self._render_balances()
        mode = "shown" if self._show_zero_balances else "hidden"
        self._set_sync(f"Zero-balance assets {mode} · no network refresh")

    def action_manage_wallets(self) -> None:
        self._open_wallet_manager()

    def _open_wallet_manager(self) -> None:
        manager = self.runtime.wallet_manager
        records = manager.list_wallets()
        if not records:
            self._open_add_wallet()
            return
        states = {record.name: manager.state(record.name) for record in records}
        self.push_screen(
            WalletManagerDialog(records, manager.storage.get_default(), states),
            self._on_wallet_action,
        )

    def _on_wallet_action(self, action: WalletAction | None) -> None:
        if action is None:
            return
        manager = self.runtime.wallet_manager
        if action.action == "add":
            self._open_add_wallet()
            return
        if not action.wallet_name:
            return
        if action.action == "use":
            manager.set_default(action.wallet_name)
            self.refresh_wallet(f"Selected wallet {action.wallet_name}")
        elif action.action == "unlock":
            manager.set_default(action.wallet_name)
            self._request_unlock(action.wallet_name)
        elif action.action == "lock":
            manager.lock()
            self.refresh_wallet(f"Locked wallet {action.wallet_name}")
        elif action.action == "fund":
            self._set_status(f"Funding {action.wallet_name} on testnet...")
            self.fund_wallet(action.wallet_name)
        elif action.action == "delete":
            record = manager.get_record(action.wallet_name)
            extra = " and encrypted signing material" if not record.watch_only else ""
            self.push_screen(
                ConfirmDialog(
                    "Delete wallet",
                    f'Delete "{record.name}" metadata{extra}? This cannot be undone.',
                    "Delete",
                ),
                lambda confirmed: self._delete_wallet(record.name, confirmed),
            )

    def _delete_wallet(self, name: str, confirmed: bool) -> None:
        if not confirmed:
            return
        try:
            self.runtime.wallet_manager.delete(name)
        except FresnicaError as exc:
            self._show_error(exc)
            return
        self.refresh_wallet(f'Deleted wallet "{name}"')

    def _open_add_wallet(self) -> None:
        self.push_screen(AddWalletDialog(), self._on_add_wallet_kind)

    def _on_add_wallet_kind(self, kind: str | None) -> None:
        if kind is None:
            return
        try:
            network = self.runtime.wallet_manager.get_record().network
        except FresnicaError:
            network = self.runtime.network
        dialogs = {
            "create": CreateWalletDialog,
            "import-secret": ImportSecretDialog,
            "import-mnemonic": ImportMnemonicDialog,
            "import-watch": WatchWalletDialog,
        }
        dialog = dialogs[kind](network)
        callbacks = {
            "create": self._on_create_request,
            "import-secret": self._on_secret_import_request,
            "import-mnemonic": self._on_mnemonic_import_request,
            "import-watch": self._on_watch_request,
        }
        self.push_screen(dialog, callbacks[kind])

    def _on_create_request(self, request) -> None:
        if request is None:
            return
        self._set_status(f'Creating wallet "{request.name}"...')
        self.create_wallet(request)

    @work(thread=True, exit_on_error=False)
    def create_wallet(self, request) -> None:
        try:
            record, mnemonic = self.runtime.wallet_manager.create_mnemonic(
                request.name,
                request.password,
                mnemonic_passphrase=request.mnemonic_passphrase,
                index=request.index,
                language=request.language,
                strength=request.strength,
                network=request.network,
            )
            self.runtime.wallet_manager.set_default(record.name)
            self.call_from_thread(self._finish_wallet_add, record, mnemonic, None)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._finish_wallet_add, None, None, exc)

    def _on_secret_import_request(self, request) -> None:
        if request is None:
            return
        self._set_status(f'Importing wallet "{request.name}"...')
        self.import_secret_wallet(request)

    @work(thread=True, exit_on_error=False)
    def import_secret_wallet(self, request) -> None:
        try:
            record = self.runtime.wallet_manager.import_secret(
                request.name,
                request.secret,
                request.password,
                network=request.network,
            )
            self.runtime.wallet_manager.set_default(record.name)
            self.call_from_thread(self._finish_wallet_add, record, None, None)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._finish_wallet_add, None, None, exc)

    def _on_mnemonic_import_request(self, request) -> None:
        if request is None:
            return
        self._set_status(f'Importing wallet "{request.name}"...')
        self.import_mnemonic_wallet(request)

    @work(thread=True, exit_on_error=False)
    def import_mnemonic_wallet(self, request) -> None:
        try:
            record = self.runtime.wallet_manager.import_mnemonic(
                request.name,
                request.mnemonic,
                request.password,
                mnemonic_passphrase=request.mnemonic_passphrase,
                index=request.index,
                language=request.language,
                network=request.network,
            )
            self.runtime.wallet_manager.set_default(record.name)
            self.call_from_thread(self._finish_wallet_add, record, None, None)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._finish_wallet_add, None, None, exc)

    def _on_watch_request(self, request) -> None:
        if request is None:
            return
        try:
            record = self.runtime.wallet_manager.add_watch(
                request.name,
                request.address,
                network=request.network,
            )
            self.runtime.wallet_manager.set_default(record.name)
        except (FresnicaError, ValueError) as exc:
            self._show_error(exc)
            return
        self._finish_wallet_add(record, None, None)

    def _finish_wallet_add(self, record, mnemonic: str | None, error) -> None:
        if error is not None:
            self._show_error(error)
            return
        self.refresh_wallet(f'Added wallet "{record.name}"')
        if mnemonic:
            self.push_screen(MnemonicBackupDialog(record.name, mnemonic))

    def action_toggle_lock(self) -> None:
        manager = self.runtime.wallet_manager
        try:
            record = manager.get_record()
            state = manager.state(record.name)
        except WalletNotFoundError:
            self._show_notice("No wallet", "Add or import a wallet before unlocking.")
            return
        if state is WalletState.WATCH_ONLY:
            self._show_notice(
                "Watch-only wallet",
                "This wallet has no signing material, so it cannot be unlocked or sign transactions.",
            )
        elif state is WalletState.UNLOCKED:
            manager.lock()
            self.refresh_wallet(f"Locked wallet {record.name}")
        else:
            self._request_unlock(record.name)

    def _request_unlock(self, wallet_name: str, after: str | None = None, error: str | None = None) -> None:
        self.push_screen(
            UnlockDialog(wallet_name, error),
            lambda password: self._on_unlock_response(wallet_name, after, password),
        )

    def _on_unlock_response(self, wallet_name: str, after: str | None, password: str | None) -> None:
        if password is None:
            return
        manager = self.runtime.wallet_manager
        try:
            manager.set_default(wallet_name)
            manager.unlock(wallet_name, password)
        except (FresnicaError, ValueError) as exc:
            self._request_unlock(wallet_name, after, str(exc))
            return
        self.refresh_wallet(f"Unlocked wallet {wallet_name}")
        if after == "send":
            self._open_send()

    def action_send(self) -> None:
        manager = self.runtime.wallet_manager
        try:
            record = manager.get_record()
            state = manager.state(record.name)
        except WalletNotFoundError:
            self._show_notice("No wallet", "Add or import a wallet before sending.")
            return
        if state is WalletState.WATCH_ONLY:
            self._show_notice(
                "Watch-only wallet",
                "This wallet can view balances, history, offers, and market data, but it cannot sign transactions.",
            )
            return
        if state is WalletState.LOCKED:
            self._request_unlock(record.name, after="send")
            return
        self._open_send()

    def _open_send(self) -> None:
        record = self.runtime.wallet_manager.get_record()
        self.push_screen(SendDialog(record.name), self._on_send_request)

    def _on_send_request(self, request) -> None:
        if request is None:
            return
        self._set_status("Preparing transaction...")
        self.prepare_send(request)

    def refresh_wallet(self, ready_message: str | None = None) -> None:
        self._set_sync("Refreshing account data...")
        self._refresh_wallet(ready_message)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_wallet(self, ready_message: str | None = None) -> None:
        record = None
        try:
            session = self.runtime.wallet_manager.view()
            record = session.record
            services = self.runtime.services_for(record.network)
            balances, positions = services.balance_service.get_portfolio_views(session.wallet)
            history = services.history_service.get_views(session.wallet, limit=20)
            self.call_from_thread(
                self._apply_wallet,
                record,
                balances,
                positions,
                history,
                ready_message,
                None,
            )
        except WalletNotFoundError:
            self.call_from_thread(self._apply_wallet, None, [], [], [], ready_message, None)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._apply_wallet, record, [], [], [], ready_message, exc)

    @work(thread=True, exit_on_error=False)
    def fund_wallet(self, wallet_name: str) -> None:
        try:
            record = self.runtime.wallet_manager.get_record(wallet_name)
            if record.network != "testnet":
                raise NetworkError("Friendbot is only available on testnet")
            services = self.runtime.services_for("testnet")
            if services.testnet_service is None:
                raise NetworkError("Friendbot is unavailable for testnet")
            result = services.testnet_service.fund(record.address)
            tx_hash = result.get("hash") if isinstance(result, dict) else None
            message = f'Funded wallet "{record.name}" on testnet'
            if tx_hash:
                message += f"; transaction {tx_hash}"
            self.call_from_thread(self.refresh_wallet, message)
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._show_error, exc)

    @work(thread=True, exit_on_error=False)
    def prepare_send(self, request) -> None:
        manager = self.runtime.wallet_manager
        try:
            record = manager.get_record()
            session = manager.current()
            if session is None or session.record.name != record.name:
                raise WalletLockedError(f'Wallet "{record.name}" is locked')
            services = self.runtime.services_for(record.network)
            destination = resolve_destination(
                getattr(self.runtime, "contact_store", None),
                request.destination,
                request.memo,
            )
            prepare_kwargs = {
                "wallet_name": record.name,
                "wallet": session.wallet,
                "destination": destination.address,
                "asset": request.asset,
                "amount": request.amount,
                "memo": destination.memo,
            }
            if destination.contact_name is not None:
                prepare_kwargs["contact_name"] = destination.contact_name
            prepared = services.transfer_service.prepare(**prepare_kwargs)
            self.call_from_thread(
                self._show_review,
                session.wallet,
                services,
                prepared,
                record.network,
            )
        except (FresnicaError, ValueError) as exc:
            self.call_from_thread(self._show_error, exc)

    def _show_review(self, wallet, services, prepared, network: str) -> None:
        self._pending_send = (wallet, services, prepared, network)
        self._set_status("Transaction ready for review")
        self.push_screen(ReviewDialog(prepared.review), self._on_review)

    def _on_review(self, confirmed: bool) -> None:
        if not confirmed:
            self._pending_send = None
            self._set_status("Transaction cancelled; wallet remains unlocked")
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
            self._pending_send = None

    def _finish_send(self, result, network: str, error) -> None:
        if error is not None:
            self._show_error(error)
            return
        message = (
            f"Submitted on {network}: {result.hash}"
            + (f"  ledger {result.ledger}" if result.ledger is not None else "")
        )
        self.refresh_wallet(message)

    def _apply_wallet(self, record, balances, positions, history, ready_message, error) -> None:
        wallet_widget = self.query_one("#wallet", Static)
        wallet_actions = self.query_one("#wallet-actions", Static)

        self._last_record = record
        self._last_balances = list(balances)
        self._last_positions = list(positions)
        self._last_history = list(history)

        if record is None:
            wallet_widget.update("No wallet configured")
            wallet_actions.update("")
            self._render_balances()
            self._render_liquidity()
            self._render_history()
            self._set_sync("No wallet selected")
            if ready_message:
                self._set_status(ready_message)
            if error is not None:
                self._show_error(error)
            return

        state = self.runtime.wallet_manager.state(record.name)
        wallet_widget.update(
            f"{record.name}\n{_wallet_meta(record, state)}\n{record.address}"
        )
        wallet_actions.update(_signing_actions_for(state))
        self._render_balances()
        self._render_liquidity()
        self._render_history()
        self._set_sync(f"Updated {datetime.now().strftime('%H:%M:%S')}")
        if ready_message:
            self._set_status(ready_message)
        if error is not None:
            self._set_sync("Account refresh failed")
            self._show_error(error)

    def _render_balances(self) -> None:
        table = self.query_one("#balances", DataTable)
        table.clear()
        items = self._last_balances
        if not self._show_zero_balances:
            items = [item for item in items if not _zero_balance(item)]
        title = "Assets · showing zero" if self._show_zero_balances else "Assets · zero hidden"
        self.query_one("#assets-title", Label).update(title)
        for item in items:
            table.add_row(
                asset_label(item.asset),
                asset_source(item.asset),
                format_amount(item.balance),
                format_amount(item.available),
                format_amount(item.selling_liabilities),
            )

    def _render_liquidity(self) -> None:
        table = self.query_one("#liquidity", DataTable)
        title = self.query_one("#liquidity-title", Label)
        table.clear()
        visible = bool(self._last_positions)
        table.display = visible
        title.display = visible
        for position in self._last_positions:
            pool_assets = " / ".join(
                asset_label(reserve.asset) for reserve in position.reserves
            ) or f"Pool {short_pool_id(position.pool_id)}"
            if position.error:
                detail = "Pool details unavailable"
            else:
                detail = " + ".join(
                    f"{format_amount(reserve.amount)} {asset_label(reserve.asset)}"
                    for reserve in position.reserves
                ) or "No reserves"
            table.add_row(
                f"{pool_assets} · {short_pool_id(position.pool_id)}",
                format_amount(position.shares),
                detail,
            )

    def _render_history(self) -> None:
        table = self.query_one("#history", DataTable)
        table.clear()
        for item in self._last_history:
            table.add_row(format_timestamp(item.created_at), item.summary)

    def _show_notice(self, title: str, message: str) -> None:
        self.push_screen(NoticeDialog(title, message))

    def _show_error(self, exc) -> None:
        self.push_screen(ErrorDialog(str(exc), getattr(exc, "details", None)))

    def _set_status(self, message: str) -> None:
        self.query_one("#status", Static).update(Text(message, style="green"))

    def _set_sync(self, message: str) -> None:
        style = "yellow" if "Refreshing" in message or "Loading" in message else "dim"
        self.query_one("#sync-status", Static).update(Text(message, style=style))


def _wallet_type(value: str) -> str:
    return {
        "watch-only": "Watch-only",
        "mnemonic": "Mnemonic wallet",
        "secret": "Secret-key wallet",
    }.get(value, value)


def _wallet_meta(record, state: WalletState) -> str:
    network = record.network.upper()
    wallet_type = _wallet_type(record.wallet_type)
    if state is WalletState.WATCH_ONLY:
        return f"{network} · {wallet_type}"
    state_label = "Unlocked" if state is WalletState.UNLOCKED else "Locked"
    return f"{network} · {wallet_type} · {state_label}"


def _signing_actions_for(state: WalletState) -> str:
    if state is WalletState.WATCH_ONLY:
        return ""
    if state is WalletState.LOCKED:
        return "S Send   L Unlock"
    return "S Send   L Lock"


def _zero_balance(item) -> bool:
    return (
        item.balance == 0
        and item.selling_liabilities == 0
        and item.buying_liabilities == 0
    )


def run_tui(runtime=None):
    if runtime is None:
        from ..runtime import Runtime

        runtime = Runtime()
    FresnicaApp(runtime).run()

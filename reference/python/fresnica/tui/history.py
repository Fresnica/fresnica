"""Full account activity screen backed by the local operation cache."""

from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Footer, Label, Select, Static

from ..errors import FresnicaError
from ..history_service import activity_counterparties, is_dust_activity
from ..presentation import format_timestamp, short_address
from .contact_book import AddContactDialog, ContactBookScreen


class AddressPickerDialog(ModalScreen[str | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    AddressPickerDialog { align: center middle; }
    AddressPickerDialog > #dialog { width: 76; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    AddressPickerDialog Select { margin: 1 0; }
    AddressPickerDialog #actions { height: auto; align-horizontal: right; }
    AddressPickerDialog Button { margin-left: 1; }
    """

    def __init__(self, addresses: list[str]):
        super().__init__()
        self.addresses = addresses

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Choose counterparty")
            yield Select(
                [(short_address(value), value) for value in self.addresses],
                value=self.addresses[0],
                allow_blank=False,
                id="address",
            )
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Add contact", id="choose", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
        elif event.button.id == "choose":
            self.dismiss(str(self.query_one("#address", Select).value))


class ActivityDetailDialog(ModalScreen[str | None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("a", "add_contact", "Add contact"),
    ]

    CSS = """
    ActivityDetailDialog { align: center middle; }
    ActivityDetailDialog > #dialog { width: 100; height: auto; max-height: 90%; padding: 1 2; border: round $accent; background: $surface; }
    #activity-detail { margin: 1 0; }
    #activity-ops { height: auto; max-height: 18; }
    #activity-actions { height: auto; margin-top: 1; align-horizontal: right; }
    """

    def __init__(self, activity, account: str, use_local_time: bool):
        super().__init__()
        self.activity = activity
        self.account = account
        self.use_local_time = use_local_time
        self.counterparties = activity_counterparties(activity, account)

    def compose(self) -> ComposeResult:
        zone = "local" if self.use_local_time else "UTC"
        transaction = self.activity.transaction_hash or "-"
        counterparties = ", ".join(short_address(item) for item in self.counterparties) or "-"
        text = (
            f"{self.activity.summary}\n"
            f"Time ({zone}): {format_timestamp(self.activity.created_at, compact=False, local=self.use_local_time)}\n"
            f"Transaction: {transaction}\n"
            f"Operations: {self.activity.operation_count}\n"
            f"Counterparties: {counterparties}"
        )
        with Vertical(id="dialog"):
            yield Label("Activity details")
            yield Static(text, id="activity-detail")
            yield DataTable(id="activity-ops")
            with Horizontal(id="activity-actions"):
                if self.counterparties:
                    yield Button("Add contact", id="add-contact")
                yield Button("Back", id="close", variant="primary")

    def on_mount(self) -> None:
        table = self.query_one("#activity-ops", DataTable)
        table.add_columns("#", "Operation", "Details")
        table.cursor_type = "row"
        for index, operation in enumerate(self.activity.operations, start=1):
            raw = operation.raw
            details = operation.summary
            source = raw.get("source_account")
            if source and source != self.account:
                details += f" · source {short_address(source)}"
            token = raw.get("paging_token") or raw.get("id")
            if token:
                details += f" · op {token}"
            table.add_row(str(index), operation.operation_type, details)

    def action_close(self) -> None:
        self.dismiss(None)

    def action_add_contact(self) -> None:
        if not self.counterparties:
            return
        self.dismiss(self.counterparties[0] if len(self.counterparties) == 1 else "__choose__")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "close":
            self.action_close()
        elif event.button.id == "add-contact":
            self.action_add_contact()


class HistoryScreen(ModalScreen[None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("r", "refresh", "Refresh"),
        Binding("m", "more", "Older"),
        Binding("d", "toggle_dust", "Dust"),
        Binding("u", "toggle_timezone", "UTC / Local"),
        Binding("enter", "details", "Details"),
        Binding("c", "contacts", "Contacts"),
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
        self._visible_views = []

    def compose(self) -> ComposeResult:
        yield Static(f"Activity · {self.wallet_name}", id="history-title")
        yield Static("Loading local activity...", id="history-status")
        yield DataTable(id="history-table")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#history-table", DataTable)
        table.add_columns(self._time_column(), "Activity")
        table.cursor_type = "row"
        self._load_cached()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_refresh(self) -> None:
        self.query_one("#history-status", Static).update("Refreshing recent activity from Horizon...")
        self._refresh_history()

    def action_more(self) -> None:
        self.query_one("#history-status", Static).update("Downloading 200 older operations from Horizon...")
        self._load_more()

    def action_toggle_dust(self) -> None:
        settings = self._settings()
        settings.show_dust_activity = not settings.show_dust_activity
        self._save_settings()
        self._load_cached()

    def action_toggle_timezone(self) -> None:
        settings = self._settings()
        settings.use_local_time = not settings.use_local_time
        self._save_settings()
        table = self.query_one("#history-table", DataTable)
        table.columns[0].label = self._time_column()
        self._load_cached()

    def action_contacts(self) -> None:
        store = getattr(self.app.runtime, "contact_store", None)
        if store is not None:
            self.app.push_screen(ContactBookScreen(store))

    def action_details(self) -> None:
        if not self._visible_views:
            return
        row = self.query_one("#history-table", DataTable).cursor_row
        activity = self._visible_views[max(0, min(row, len(self._visible_views) - 1))]
        self.app.push_screen(
            ActivityDetailDialog(
                activity,
                self.wallet.address(),
                self._settings().use_local_time,
            ),
            lambda result: self._detail_result(activity, result),
        )

    def _detail_result(self, activity, result: str | None) -> None:
        if result is None:
            return
        addresses = activity_counterparties(activity, self.wallet.address())
        if result == "__choose__":
            self.app.push_screen(
                AddressPickerDialog(addresses),
                lambda address: self._open_add_contact(address),
            )
            return
        self._open_add_contact(result)

    def _open_add_contact(self, address: str | None) -> None:
        store = getattr(self.app.runtime, "contact_store", None)
        if store is not None and address:
            self.app.push_screen(AddContactDialog(store, address))

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _load_cached(self) -> None:
        try:
            views = self.history_service.get_activity_views(
                self.wallet,
                limit=100000,
                refresh=False,
            )
            count = self.history_service.cached_operation_count(self.wallet)
            self.app.call_from_thread(self._apply, views, count, "Local activity", None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply, [], 0, "", exc)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_history(self) -> None:
        try:
            self.history_service.sync_recent(self.wallet)
            views = self.history_service.get_activity_views(
                self.wallet,
                limit=100000,
                refresh=False,
            )
            count = self.history_service.cached_operation_count(self.wallet)
            self.app.call_from_thread(self._apply, views, count, "Activity updated", None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply, [], 0, "", exc)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _load_more(self) -> None:
        try:
            added = self.history_service.load_older(self.wallet, limit=200)
            self.limit += 200
            views = self.history_service.get_activity_views(
                self.wallet,
                limit=100000,
                refresh=False,
            )
            count = self.history_service.cached_operation_count(self.wallet)
            message = f"Cached {added} older operations" if added else "No older operations returned"
            self.app.call_from_thread(self._apply, views, count, message, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply, [], 0, "", exc)

    def _apply(self, views, cached_operations: int, message: str, error) -> None:
        table = self.query_one("#history-table", DataTable)
        table.clear()
        settings = self._settings()
        filtered = views if settings.show_dust_activity else [item for item in views if not is_dust_activity(item)]
        self._visible_views = list(filtered[: self.limit])
        for item in self._visible_views:
            table.add_row(
                format_timestamp(item.created_at, local=settings.use_local_time),
                item.summary,
            )
        status = self.query_one("#history-status", Static)
        if error is not None:
            details = getattr(error, "details", None)
            text = f"ERROR {error}"
            if details:
                text += f" · DEV {details}"
            status.update(text)
            return
        dust = "shown" if settings.show_dust_activity else "hidden"
        zone = "local time" if settings.use_local_time else "UTC"
        status.update(
            f"{message} · {len(self._visible_views)} activities shown · "
            f"{cached_operations} operations cached · dust {dust} · {zone}"
        )

    def _settings(self):
        return self.app.runtime.settings

    def _save_settings(self) -> None:
        self.app.runtime.settings_store.save(self.app.runtime.settings)

    def _time_column(self) -> str:
        settings = self._settings()
        return "Time (local)" if settings.use_local_time else "Time (UTC)"

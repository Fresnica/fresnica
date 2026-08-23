"""Full account activity screen backed by the local operation cache."""

from rich.text import Text
from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical, VerticalScroll
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Footer, Label, Select, Static

from ..errors import FresnicaError
from ..history_service import activity_counterparties, is_suspicious_claimable_activity
from ..presentation import format_timestamp, short_address
from ..settings import UserSettings
from .activity_presentation import activity_display_summary, activity_metadata, activity_text
from .contact_book import AddContactDialog, ContactBookScreen
from .list_search import ListSearchDialog, matches_query


class AddressPickerDialog(ModalScreen[str | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    AddressPickerDialog { align: center middle; }
    AddressPickerDialog > #dialog { width: 76; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    AddressPickerDialog Select { margin: 1 0; }
    AddressPickerDialog #actions { height: auto; align-horizontal: right; }
    AddressPickerDialog Button { margin-left: 1; }
    """

    def __init__(self, addresses: list[str], contact_names: dict[str, str] | None = None):
        super().__init__()
        self.addresses = addresses
        self.contact_names = contact_names or {}

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Choose counterparty")
            yield Select(
                [(_address_label(value, self.contact_names), value) for value in self.addresses],
                value=self.addresses[0],
                allow_blank=False,
                id="address",
            )
            with Horizontal(id="actions"):
                yield Button("Cancel [Esc]", id="cancel")
                yield Button("Add contact [A]", id="choose", variant="primary")

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
    #activity-ops { height: auto; min-height: 3; max-height: 18; margin-top: 1; }
    .activity-op { width: 100%; height: auto; margin-bottom: 1; }
    #activity-actions { height: auto; margin-top: 1; align-horizontal: right; }
    """

    def __init__(
        self,
        activity,
        account: str,
        use_local_time: bool,
        summary: str | None = None,
        contact_names: dict[str, str] | None = None,
    ):
        super().__init__()
        self.activity = activity
        self.account = account
        self.use_local_time = use_local_time
        self.summary = summary or activity.summary
        self.contact_names = contact_names or {}
        self.operations = list(getattr(activity, "operations", []) or [activity])
        self.counterparties = (
            activity_counterparties(activity, account)
            if hasattr(activity, "operations")
            else []
        )

    def compose(self) -> ComposeResult:
        zone = "local" if self.use_local_time else "UTC"
        raw = getattr(self.activity, "raw", {})
        transaction = getattr(self.activity, "transaction_hash", None)
        if not transaction and isinstance(raw, dict):
            transaction = raw.get("transaction_hash")
        counterparties = ", ".join(
            _address_label(item, self.contact_names) for item in self.counterparties
        ) or "-"
        text = (
            f"{self.summary}\n"
            f"Time ({zone}): {format_timestamp(self.activity.created_at, compact=False, local=self.use_local_time)}\n"
            f"Transaction: {transaction or '-'}\n"
            f"Operations: {len(self.operations)}\n"
            f"Counterparties: {counterparties}"
        )
        with Vertical(id="dialog"):
            yield Label("Activity details")
            yield Static(text, id="activity-detail")
            with VerticalScroll(id="activity-ops"):
                for index, operation in enumerate(self.operations, start=1):
                    yield Static(self._operation_detail(index, operation), classes="activity-op")
            with Horizontal(id="activity-actions"):
                if self.counterparties:
                    yield Button("Add contact [A]", id="add-contact")
                yield Button("Back [Esc]", id="close", variant="primary")

    def _operation_detail(self, index: int, operation) -> Text:
        raw = operation.raw
        details = operation.summary
        source = raw.get("source_account")
        if source and source != self.account:
            details += f" · source {_address_label(source, self.contact_names)}"
        changes = raw.get("asset_balance_changes", []) or []
        if changes:
            details += f" · {len(changes)} asset change{'s' if len(changes) != 1 else ''}"
        token = raw.get("paging_token") or raw.get("id")
        if token:
            details += f" · op {token}"
        text = Text()
        text.append(f"#{index} {_operation_label(operation.operation_type)}", style="bold")
        text.append("\n")
        text.append(details)
        return text

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
        Binding("p", "toggle_suspicious", "Suspicious"),
        Binding("u", "toggle_timezone", "UTC / Local"),
        Binding("enter", "details", "Details"),
        Binding("c", "contacts", "Contacts"),
        Binding("/", "search", "Search"),
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
        self._fallback_settings = UserSettings()
        self._time_column_key = None
        self._search_query = ""
        self._loaded_views = []
        self._loaded_count = 0
        self._loaded_message = ""
        self._loaded_error = None

    def compose(self) -> ComposeResult:
        yield Static(f"Activity · {self.wallet_name}", id="history-title")
        yield Static("Loading local activity...", id="history-status")
        yield DataTable(id="history-table")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#history-table", DataTable)
        if not table.columns:
            self._time_column_key = table.add_column(self._time_column())
            table.add_column("Activity")
        elif self._time_column_key is None:
            self._time_column_key = next(iter(table.columns.keys()), None)
        table.cursor_type = "row"
        if not self._visible_views:
            self._load_cached()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_search(self) -> None:
        self.app.push_screen(
            ListSearchDialog(
                self._search_query,
                on_change=self._set_search_query,
                label="Search activity",
            ),
            lambda _: self.call_later(
                lambda: self.set_focus(self.query_one("#history-table", DataTable))
            ),
        )

    def _set_search_query(self, query: str) -> None:
        self._search_query = query
        self._render_loaded()

    def action_refresh(self) -> None:
        self.query_one("#history-status", Static).update("Refreshing recent activity from Horizon...")
        self._refresh_history()

    def action_more(self) -> None:
        self.query_one("#history-status", Static).update("Downloading 200 older operations from Horizon...")
        self._load_more()

    def action_toggle_suspicious(self) -> None:
        settings = self._settings()
        settings.hide_suspicious_claimables = not settings.hide_suspicious_claimables
        self._save_settings()
        self._load_cached()

    def action_toggle_timezone(self) -> None:
        settings = self._settings()
        settings.use_local_time = not settings.use_local_time
        self._save_settings()
        table = self.query_one("#history-table", DataTable)
        if self._time_column_key is not None:
            column = table.columns.get(self._time_column_key)
            if column is not None:
                column.label = Text(self._time_column())
                table.refresh()
        self._load_cached()

    def action_contacts(self) -> None:
        store = getattr(self.app.runtime, "contact_store", None)
        if store is not None:
            self.app.push_screen(ContactBookScreen(store), lambda _: self._load_cached())

    def action_details(self) -> None:
        if not self._visible_views:
            return
        row = self.query_one("#history-table", DataTable).cursor_row
        activity = self._visible_views[max(0, min(row, len(self._visible_views) - 1))]
        contacts, domains = activity_metadata(self.app.runtime, self.wallet)
        summary = activity_display_summary(
            activity,
            self.wallet.address(),
            contacts,
            domains,
        )
        self.app.push_screen(
            ActivityDetailDialog(
                activity,
                self.wallet.address(),
                self._settings().use_local_time,
                summary=summary,
                contact_names=contacts,
            ),
            lambda result: self._detail_result(activity, result),
        )

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        if event.data_table.id == "history-table":
            self.action_details()

    def _detail_result(self, activity, result: str | None) -> None:
        if result is None or not hasattr(activity, "operations"):
            return
        addresses = activity_counterparties(activity, self.wallet.address())
        if result == "__choose__":
            contacts, _ = activity_metadata(self.app.runtime, self.wallet)
            self.app.push_screen(
                AddressPickerDialog(addresses, contacts),
                lambda address: self._open_add_contact(address),
            )
            return
        self._open_add_contact(result)

    def _open_add_contact(self, address: str | None) -> None:
        store = getattr(self.app.runtime, "contact_store", None)
        if store is not None and address:
            self.app.push_screen(AddContactDialog(store, address), self._after_add_contact)

    def _after_add_contact(self, contact) -> None:
        if contact is not None:
            self._load_cached()

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _load_cached(self) -> None:
        try:
            views = self.history_service.get_activity_views(
                self.wallet,
                limit=100000,
                refresh=False,
            )
            count = self._cached_operation_count(views)
            self.app.call_from_thread(self._apply, views, count, "Local activity", None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply, [], 0, "", exc)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_history(self) -> None:
        try:
            sync_recent = getattr(self.history_service, "sync_recent", None)
            if sync_recent is not None:
                sync_recent(self.wallet)
                views = self.history_service.get_activity_views(
                    self.wallet,
                    limit=100000,
                    refresh=False,
                )
            else:
                views = self.history_service.get_activity_views(
                    self.wallet,
                    limit=100000,
                    refresh=True,
                )
            count = self._cached_operation_count(views)
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
            count = self._cached_operation_count(views)
            message = f"Cached {added} older operations" if added else "No older operations returned"
            self.app.call_from_thread(self._apply, views, count, message, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply, [], 0, "", exc)

    def _cached_operation_count(self, views) -> int:
        counter = getattr(self.history_service, "cached_operation_count", None)
        return counter(self.wallet) if counter is not None else len(views)

    def _apply(self, views, cached_operations: int, message: str, error) -> None:
        self._loaded_views = list(views)
        self._loaded_count = cached_operations
        self._loaded_message = message
        self._loaded_error = error
        self._render_loaded()

    def _render_loaded(self) -> None:
        views = self._loaded_views
        cached_operations = self._loaded_count
        message = self._loaded_message
        error = self._loaded_error
        table = self.query_one("#history-table", DataTable)
        table.clear()
        settings = self._settings()
        if settings.hide_suspicious_claimables:
            filtered = [
                item
                for item in views
                if not (
                    hasattr(item, "operations")
                    and is_suspicious_claimable_activity(item)
                )
            ]
        else:
            filtered = views
        contacts, domains = activity_metadata(self.app.runtime, self.wallet)
        account = self.wallet.address()
        if self._search_query:
            searched = []
            for item in filtered:
                summary = activity_display_summary(item, account, contacts, domains)
                if matches_query(
                    self._search_query,
                    summary,
                    activity_text(item, summary, account),
                    getattr(item, "transaction_hash", None),
                    getattr(item, "raw", None),
                ):
                    searched.append(item)
            filtered = searched
        self._visible_views = list(filtered[: self.limit])
        for item in self._visible_views:
            summary = activity_display_summary(item, account, contacts, domains)
            table.add_row(
                format_timestamp(item.created_at, local=settings.use_local_time),
                activity_text(item, summary, account),
            )
        status = self.query_one("#history-status", Static)
        if error is not None:
            details = getattr(error, "details", None)
            text = f"ERROR {error}"
            if details:
                text += f" · DEV {details}"
            status.update(text)
            return
        suspicious = "hidden" if settings.hide_suspicious_claimables else "shown (dimmed)"
        zone = "local time" if settings.use_local_time else "UTC"
        status.update(
            f"{message} · {len(self._visible_views)} activities shown · / search · "
            f"{cached_operations} operations cached · suspicious claimables {suspicious} · {zone}"
        )

    def _settings(self):
        return getattr(self.app.runtime, "settings", self._fallback_settings)

    def _save_settings(self) -> None:
        settings = getattr(self.app.runtime, "settings", None)
        store = getattr(self.app.runtime, "settings_store", None)
        if settings is not None and store is not None:
            store.save(settings)

    def _time_column(self) -> str:
        settings = self._settings()
        return "Time (local)" if settings.use_local_time else "Time (UTC)"


def _address_label(address: str, contact_names: dict[str, str]) -> str:
    name = contact_names.get(address)
    return f"👤 {name} · {short_address(address)}" if name else short_address(address)


def _operation_label(kind: str) -> str:
    return {
        "invoke_host_function": "Contract call",
        "create_claimable_balance": "Claimable balance",
        "claim_claimable_balance": "Claim claimable balance",
        "clawback_claimable_balance": "Clawback claimable balance",
    }.get(kind, kind.replace("_", " ").title())

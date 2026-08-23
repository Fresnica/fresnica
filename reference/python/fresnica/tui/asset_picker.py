"""Reusable cache-first asset picker for trustlines and DEX markets."""

from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Input, Label, Static

from ..asset_catalog import AssetCatalogEntry
from ..errors import FresnicaError
from ..models import Asset
from ..presentation import short_address


class AssetPickerDialog(ModalScreen[Asset | None]):
    BINDINGS = [
        Binding("escape", "cancel", "Cancel"),
        Binding("enter", "choose", "Choose"),
        Binding("r", "refresh", "Refresh"),
    ]

    CSS = """
    AssetPickerDialog { align: center middle; }
    AssetPickerDialog > #dialog { width: 100; height: 82%; padding: 1 2; border: round $accent; background: $surface; }
    #asset-picker-status { height: 2; color: $text-muted; }
    #asset-picker-table { height: 1fr; min-height: 8; }
    #manual-title { height: 1; margin-top: 1; text-style: bold; }
    #manual-asset { margin-top: 1; }
    #asset-picker-error { height: 1; color: $error; }
    #asset-picker-actions { height: auto; align-horizontal: right; }
    #asset-picker-actions Button { margin-left: 1; }
    """

    def __init__(self, runtime, *, allow_native: bool = True, title: str = "Choose asset"):
        super().__init__()
        self.runtime = runtime
        self.allow_native = allow_native
        self.title = title
        self._entries: list[AssetCatalogEntry] = []

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(self.title)
            yield Static(
                "Recommended assets are cached locally. Full issuer identity remains authoritative.",
                id="asset-picker-status",
            )
            yield DataTable(id="asset-picker-table")
            yield Label("Manual asset", id="manual-title")
            yield Input(
                placeholder="XLM or CODE:GISSUER..." if self.allow_native else "CODE:GISSUER...",
                id="manual-asset",
            )
            yield Static("", id="asset-picker-error")
            with Horizontal(id="asset-picker-actions"):
                yield Button("Cancel [Esc]", id="cancel")
                yield Button("Use manual", id="manual")
                yield Button("Choose [Enter]", id="choose", variant="primary")

    def on_mount(self) -> None:
        table = self.query_one("#asset-picker-table", DataTable)
        if not table.columns:
            table.add_columns("Asset", "Domain / issuer", "Name", "Source")
        table.cursor_type = "row"
        self._render(self._cached_entries())
        self._refresh_catalog()

    def action_cancel(self) -> None:
        self.dismiss(None)

    def action_choose(self) -> None:
        if not self._entries:
            return
        table = self.query_one("#asset-picker-table", DataTable)
        index = max(0, min(table.cursor_row, len(self._entries) - 1))
        self.dismiss(self._entries[index].asset)

    def action_refresh(self) -> None:
        self.query_one("#asset-picker-status", Static).update("Refreshing recommended assets...")
        self._refresh_catalog()

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        if event.data_table.id == "asset-picker-table":
            self.action_choose()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
        elif event.button.id == "choose":
            self.action_choose()
        elif event.button.id == "manual":
            self._use_manual()

    def _use_manual(self) -> None:
        value = self.query_one("#manual-asset", Input).value.strip()
        error = self.query_one("#asset-picker-error", Static)
        try:
            asset = Asset.parse(value)
        except (FresnicaError, ValueError) as exc:
            error.update(str(exc))
            return
        if asset.is_liquidity_pool:
            error.update("Liquidity-pool shares cannot be selected here.")
            return
        if asset.is_native and not self.allow_native:
            error.update("This action requires an issued asset CODE:GISSUER.")
            return
        self.dismiss(asset)

    def _cached_entries(self) -> list[AssetCatalogEntry]:
        store = getattr(self.runtime, "asset_catalog", None)
        if store is None:
            return []
        try:
            record = self.runtime.wallet_manager.get_record()
            entries = store.cached(record.network)
        except (FresnicaError, ValueError):
            return []
        return self._filter(entries)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_catalog(self) -> None:
        store = getattr(self.runtime, "asset_catalog", None)
        if store is None:
            return
        try:
            record = self.runtime.wallet_manager.get_record()
            entries = store.recommended(record.network, limit=30, refresh=True)
            self.app.call_from_thread(self._apply_refresh, self._filter(entries), None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply_refresh, [], exc)

    def _apply_refresh(self, entries: list[AssetCatalogEntry], error) -> None:
        if not self.is_mounted:
            return
        if entries:
            self._render(entries)
        status = self.query_one("#asset-picker-status", Static)
        if error is not None:
            status.update("Recommended assets unavailable · cached/manual selection remains available")
        elif entries:
            status.update(f"{len(entries)} recommended assets · R refresh · full issuer identity retained")
        elif not self._entries:
            status.update("No recommended assets cached · enter a full asset identity below")

    def _filter(self, entries: list[AssetCatalogEntry]) -> list[AssetCatalogEntry]:
        return [
            item
            for item in entries
            if not item.asset.is_liquidity_pool and (self.allow_native or not item.asset.is_native)
        ]

    def _render(self, entries: list[AssetCatalogEntry]) -> None:
        self._entries = list(entries)
        table = self.query_one("#asset-picker-table", DataTable)
        table.clear()
        for entry in self._entries:
            asset = entry.asset
            if asset.is_native:
                source = "Stellar native"
            else:
                source = entry.domain or short_address(asset.issuer)
            table.add_row(
                asset.display,
                source,
                entry.name or entry.org or "",
                entry.source,
            )

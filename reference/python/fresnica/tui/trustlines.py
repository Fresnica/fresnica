"""Trustline management UI for the state-driven Fresnica TUI."""

from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from typing import Literal

from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, DataTable, Footer, Input, Label, Static

from ..errors import FresnicaError
from ..models import Asset
from ..presentation import format_amount


TrustlineActionKind = Literal["add", "limit", "remove"]


@dataclass(frozen=True)
class TrustlineAction:
    kind: TrustlineActionKind
    asset: str
    limit: str | None = None


class TrustlineFormDialog(ModalScreen[TrustlineAction | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    TrustlineFormDialog { align: center middle; }
    TrustlineFormDialog > #dialog { width: 88; height: auto; padding: 1 2; border: round $accent; background: $surface; }
    TrustlineFormDialog Input { margin-top: 1; }
    TrustlineFormDialog #asset-label { color: $text-muted; margin-top: 1; }
    TrustlineFormDialog #form-error { color: $error; margin-top: 1; }
    TrustlineFormDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    TrustlineFormDialog Button { margin-left: 1; }
    """

    def __init__(
        self,
        kind: Literal["add", "limit"],
        asset: str | None = None,
        limit: str | None = None,
    ):
        super().__init__()
        self.kind = kind
        self.asset = asset
        self.limit = limit or ""

    def compose(self) -> ComposeResult:
        title = "Add Stellar trustline" if self.kind == "add" else "Change trustline limit"
        with Vertical(id="dialog"):
            yield Label(title)
            if self.kind == "add":
                yield Input(placeholder="Asset: CODE:GISSUER", id="asset")
                yield Input(
                    value=self.limit,
                    placeholder="Limit (blank = Stellar maximum)",
                    id="limit",
                )
            else:
                yield Static(self.asset or "", id="asset-label")
                yield Input(value=self.limit, placeholder="New limit", id="limit")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Review", id="review", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "review":
            return

        error = self.query_one("#form-error", Static)
        asset_text = (
            self.query_one("#asset", Input).value.strip()
            if self.kind == "add"
            else (self.asset or "")
        )
        try:
            asset = Asset.parse(asset_text)
        except (FresnicaError, ValueError) as exc:
            error.update(str(exc))
            return
        if asset.is_native or asset.is_liquidity_pool:
            error.update("Trustlines require an issued asset CODE:GISSUER.")
            return

        limit = self.query_one("#limit", Input).value.strip()
        if self.kind == "limit" and not limit:
            error.update("A new trustline limit is required.")
            return
        if limit:
            try:
                parsed = Decimal(limit)
            except InvalidOperation:
                error.update("Trustline limit must be a decimal number.")
                return
            if not parsed.is_finite() or parsed <= 0:
                error.update("Trustline limit must be greater than zero.")
                return

        self.dismiss(
            TrustlineAction(
                kind=self.kind,
                asset=_asset_identity(asset),
                limit=limit or None,
            )
        )


class TrustlineScreen(ModalScreen[None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("r", "refresh", "Refresh"),
        Binding("a", "add", "Add"),
        Binding("e", "edit", "Set limit"),
        Binding("x", "remove", "Remove"),
    ]

    CSS = """
    TrustlineScreen { layout: vertical; background: $surface; padding: 1 2; }
    #trust-title { height: auto; text-style: bold; }
    #trust-status { height: 1; color: $text-muted; margin-bottom: 1; }
    #trustlines { height: 1fr; min-height: 10; }
    """

    def __init__(self, runtime, on_trustline_action):
        super().__init__()
        self.runtime = runtime
        self.on_trustline_action = on_trustline_action
        self._visible_lines: list[dict] = []

    def compose(self) -> ComposeResult:
        yield Static("Stellar trustlines", id="trust-title")
        yield Static("Loading trustlines...", id="trust-status")
        yield DataTable(id="trustlines")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#trustlines", DataTable)
        table.add_columns("Asset", "Balance", "Limit", "Buying liabilities", "Selling liabilities")
        table.cursor_type = "row"
        self.refresh_trustlines()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_refresh(self) -> None:
        self.refresh_trustlines()

    def action_add(self) -> None:
        self.app.push_screen(
            TrustlineFormDialog("add"),
            self._on_form_action,
        )

    def action_edit(self) -> None:
        raw = self._selected_line()
        if raw is None:
            self.set_status("Select a trustline before changing its limit.")
            return
        asset = _raw_asset_identity(raw)
        self.app.push_screen(
            TrustlineFormDialog("limit", asset=asset, limit=str(raw.get("limit", ""))),
            self._on_form_action,
        )

    def action_remove(self) -> None:
        raw = self._selected_line()
        if raw is None:
            self.set_status("Select a trustline before removing it.")
            return
        self.on_trustline_action(
            self,
            TrustlineAction(kind="remove", asset=_raw_asset_identity(raw)),
        )

    def _on_form_action(self, action: TrustlineAction | None) -> None:
        if action is not None:
            self.on_trustline_action(self, action)

    def _selected_line(self) -> dict | None:
        if not self._visible_lines:
            return None
        table = self.query_one("#trustlines", DataTable)
        index = max(0, min(table.cursor_row, len(self._visible_lines) - 1))
        return self._visible_lines[index]

    def refresh_trustlines(self) -> None:
        self.set_status("Refreshing trustlines...")
        self._refresh_trustlines()

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_trustlines(self) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            services = self.runtime.services_for(session.record.network)
            account = services.balance_service.get_account(session.wallet, refresh=True)
            lines = [
                raw
                for raw in account.get("balances", [])
                if raw.get("asset_type") not in {"native", "liquidity_pool_shares"}
            ]
            lines.sort(key=lambda raw: (str(raw.get("asset_code", "")), str(raw.get("asset_issuer", ""))))
            self.app.call_from_thread(self._apply_trustlines, lines, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply_trustlines, [], exc)

    def _apply_trustlines(self, lines, error) -> None:
        table = self.query_one("#trustlines", DataTable)
        table.clear()
        self._visible_lines = list(lines)
        for raw in lines:
            table.add_row(
                _raw_asset_identity(raw),
                format_amount(Decimal(str(raw.get("balance", "0")))),
                format_amount(Decimal(str(raw.get("limit", "0")))),
                format_amount(Decimal(str(raw.get("buying_liabilities", "0")))),
                format_amount(Decimal(str(raw.get("selling_liabilities", "0")))),
            )
        if error is not None:
            details = getattr(error, "details", None)
            text = f"ERROR {error}"
            if details:
                text += f" · DEV {details}"
            self.set_status(text)
            return
        self.set_status(f"{len(lines)} trustlines · A add · E set limit · X remove")

    def set_status(self, message: str) -> None:
        if self.is_mounted:
            self.query_one("#trust-status", Static).update(message)


def _raw_asset_identity(raw: dict) -> str:
    return f"{raw.get('asset_code', '?')}:{raw.get('asset_issuer', '?')}"


def _asset_identity(asset: Asset) -> str:
    return f"{asset.code}:{asset.issuer}"

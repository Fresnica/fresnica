"""Asset-level wallet actions and lazy anchor capability presentation."""

from dataclasses import dataclass
from typing import Literal

from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, Footer, Input, Label, Static

from ..anchor_service import AnchorCapabilities, AnchorService
from ..balance_service import ISSUER_DOMAIN_CACHE_KEY
from ..errors import FresnicaError
from ..models import BalanceView
from ..presentation import asset_source, format_amount, short_address
from .screens import SendDialog


AssetDetailActionKind = Literal["send", "receive", "trustline"]


@dataclass(frozen=True)
class AssetDetailAction:
    kind: AssetDetailActionKind
    asset: str


class PrefilledSendDialog(SendDialog):
    """Existing Send flow with only the selected asset prefilled."""

    def __init__(self, wallet_name: str, asset: str):
        super().__init__(wallet_name)
        self.initial_asset = asset

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(f"Send from {self.wallet_name}")
            yield Input(placeholder="Amount", id="amount")
            yield Input(
                value=self.initial_asset,
                placeholder="Asset (XLM or CODE:GISSUER...)",
                id="asset",
            )
            yield Input(placeholder="Destination G...", id="destination")
            yield Input(placeholder="Memo (optional)", id="memo")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel", id="cancel")
                yield Button("Review", id="review", variant="primary")


class AssetDetailsScreen(ModalScreen[AssetDetailAction | None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("s", "send", "Send"),
        Binding("r", "receive", "Receive"),
        Binding("t", "trustline", "Trustline"),
    ]

    CSS = """
    AssetDetailsScreen { align: center middle; }
    AssetDetailsScreen > #asset-dialog { width: 100; height: auto; max-height: 90%; padding: 1 2; border: round $accent; background: $surface; }
    #asset-identity { text-style: bold; }
    #asset-balance, #asset-trustline, #asset-anchor { margin-top: 1; }
    #asset-anchor { color: $text-muted; }
    #asset-actions { height: auto; margin-top: 1; align-horizontal: right; }
    #asset-actions Button { margin-left: 1; }
    """

    def __init__(self, balance: BalanceView):
        super().__init__()
        self.balance = balance
        self.asset = balance.asset

    def compose(self) -> ComposeResult:
        identity = _asset_identity(self.balance)
        source = asset_source(
            self.asset,
            str(self.balance.raw.get(ISSUER_DOMAIN_CACHE_KEY) or "") or None,
        )
        with Vertical(id="asset-dialog"):
            yield Label("Asset details")
            yield Static(identity, id="asset-identity")
            yield Static(
                f"Source: {source}\n"
                f"Balance: {format_amount(self.balance.balance)}\n"
                f"Available: {format_amount(self.balance.available)}\n"
                f"In offers: {format_amount(self.balance.selling_liabilities)}",
                id="asset-balance",
            )
            yield Static(_trustline_text(self.balance), id="asset-trustline")
            yield Static(_anchor_initial_text(self.balance), id="asset-anchor")
            with Horizontal(id="asset-actions"):
                if not self.asset.is_liquidity_pool:
                    yield Button("Receive", id="receive")
                    yield Button("Send", id="send", variant="primary")
                if not self.asset.is_native and not self.asset.is_liquidity_pool:
                    yield Button("Trustline", id="trustline")
                yield Button("Back", id="close")
            yield Footer()

    def on_mount(self) -> None:
        domain = str(self.balance.raw.get(ISSUER_DOMAIN_CACHE_KEY) or "").strip()
        if domain and not self.asset.is_native and not self.asset.is_liquidity_pool:
            self._discover_anchor(domain)

    def action_close(self) -> None:
        self.dismiss(None)

    def action_send(self) -> None:
        if not self.asset.is_liquidity_pool:
            self.dismiss(AssetDetailAction("send", _asset_identity(self.balance)))

    def action_receive(self) -> None:
        if not self.asset.is_liquidity_pool:
            self.dismiss(AssetDetailAction("receive", _asset_identity(self.balance)))

    def action_trustline(self) -> None:
        if not self.asset.is_native and not self.asset.is_liquidity_pool:
            self.dismiss(AssetDetailAction("trustline", _asset_identity(self.balance)))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        actions = {
            "close": self.action_close,
            "send": self.action_send,
            "receive": self.action_receive,
            "trustline": self.action_trustline,
        }
        action = actions.get(event.button.id)
        if action is not None:
            action()

    @work(thread=True, exit_on_error=False)
    def _discover_anchor(self, domain: str) -> None:
        try:
            capabilities = AnchorService().discover(self.asset, domain)
            self.app.call_from_thread(self._apply_anchor, capabilities, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply_anchor, None, exc)

    def _apply_anchor(self, capabilities: AnchorCapabilities | None, error) -> None:
        if not self.is_mounted:
            return
        widget = self.query_one("#asset-anchor", Static)
        if error is not None:
            widget.update(f"Anchor discovery: unavailable ({error})")
            return
        if capabilities is None:
            return
        parts = [f"Anchor: {capabilities.domain}"]
        if capabilities.sep6_url:
            methods = _methods(capabilities.sep6_deposit, capabilities.sep6_withdraw)
            parts.append(f"SEP-6: {methods} · {capabilities.sep6_url}")
        if capabilities.sep24_url:
            methods = _methods(capabilities.sep24_deposit, capabilities.sep24_withdraw)
            parts.append(f"SEP-24: {methods} · {capabilities.sep24_url}")
        if capabilities.web_auth_url:
            parts.append(f"SEP-10 auth: {capabilities.web_auth_url}")
        if capabilities.direct_payment_url:
            parts.append(f"SEP-31: {capabilities.direct_payment_url}")
        parts.extend(f"Note: {warning}" for warning in capabilities.warnings)
        if len(parts) == 1:
            parts.append("No SEP-6/SEP-24 transfer service advertised")
        widget.update("\n".join(parts))


def _asset_identity(balance: BalanceView) -> str:
    asset = balance.asset
    if asset.is_native:
        return "XLM"
    if asset.is_liquidity_pool:
        return f"LP:{asset.liquidity_pool_id or ''}"
    return f"{asset.code}:{asset.issuer}"


def _trustline_text(balance: BalanceView) -> str:
    asset = balance.asset
    if asset.is_native:
        return "Native XLM · no trustline"
    if asset.is_liquidity_pool:
        return "Liquidity-pool share"
    raw = balance.raw
    lines = [f"Trustline limit: {format_amount(raw.get('limit', '?'))}"]
    if "is_authorized" in raw:
        lines.append(f"Authorized: {'yes' if raw.get('is_authorized') else 'no'}")
    if raw.get("is_authorized_to_maintain_liabilities"):
        lines.append("Maintain liabilities: yes")
    if raw.get("is_clawback_enabled"):
        lines.append("Clawback enabled: yes")
    lines.append(f"Issuer: {short_address(asset.issuer)}")
    return "\n".join(lines)


def _anchor_initial_text(balance: BalanceView) -> str:
    if balance.asset.is_native or balance.asset.is_liquidity_pool:
        return ""
    domain = str(balance.raw.get(ISSUER_DOMAIN_CACHE_KEY) or "").strip()
    return f"Anchor discovery: checking {domain}..." if domain else "Anchor discovery: issuer has no home_domain"


def _methods(deposit: bool, withdraw: bool) -> str:
    methods = []
    if deposit:
        methods.append("deposit")
    if withdraw:
        methods.append("withdraw")
    return "/".join(methods) if methods else "advertised; asset methods unavailable"

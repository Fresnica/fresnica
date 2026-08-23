"""Asset-level wallet actions and explicit anchor transfer presentation."""

from dataclasses import dataclass
from typing import Literal
import webbrowser

from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, Footer, Input, Label, Static

from ..anchor_service import AnchorCapabilities, AnchorService
from ..balance_service import ISSUER_DOMAIN_CACHE_KEY
from ..errors import FresnicaError, WalletLockedError
from ..manager import WalletState
from ..models import BalanceView
from ..network import get_network
from ..presentation import asset_source, format_amount, short_address
from .screens import SendDialog, UnlockDialog
from .trustlines import TrustlineAction, TrustlineFormDialog


AssetDetailActionKind = Literal["send"]


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
            yield Input(placeholder="Destination or contact", id="destination")
            yield Input(placeholder="Memo (optional)", id="memo")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel [Esc]", id="cancel")
                yield Button("Review", id="review", variant="primary")


class AssetDetailsScreen(ModalScreen[AssetDetailAction | None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("s", "send", "Send"),
        Binding("l", "set_limit", "Set limit"),
        Binding("x", "remove_trustline", "Remove"),
        Binding("a", "anchor", "Anchor"),
        Binding("d", "anchor_deposit", "Deposit"),
        Binding("w", "anchor_withdraw", "Withdraw"),
    ]

    CSS = """
    AssetDetailsScreen { align: center middle; }
    AssetDetailsScreen > #asset-dialog { width: 104; height: auto; max-height: 88%; padding: 1 2; border: round $accent; background: $surface; }
    #asset-identity { text-style: bold; }
    #asset-balance, #asset-trustline, #asset-anchor { margin-top: 1; }
    #asset-anchor { color: $text-muted; }
    #asset-status { height: 1; margin-top: 1; color: $text-muted; }
    #asset-actions { height: auto; margin-top: 1; align-horizontal: right; }
    #asset-actions Button { margin-left: 1; }
    """

    def __init__(self, balance: BalanceView, runtime=None, on_trustline_action=None):
        super().__init__()
        self.balance = balance
        self.asset = balance.asset
        self.runtime = runtime
        self.on_trustline_action = on_trustline_action
        self.domain = str(balance.raw.get(ISSUER_DOMAIN_CACHE_KEY) or "").strip()
        self._anchor_loading = False
        self._anchor_capabilities: AnchorCapabilities | None = None

    def compose(self) -> ComposeResult:
        with Vertical(id="asset-dialog"):
            yield Label("Asset details")
            yield Static("", id="asset-identity")
            yield Static("", id="asset-balance")
            yield Static("", id="asset-trustline")
            yield Static(_anchor_initial_text(self.balance), id="asset-anchor")
            yield Static("", id="asset-status")
            with Horizontal(id="asset-actions"):
                if not self.asset.is_liquidity_pool:
                    yield Button("Send [S]", id="send", variant="primary")
                if not self.asset.is_native and not self.asset.is_liquidity_pool:
                    yield Button("Set limit [L]", id="set-limit")
                    yield Button("Remove [X]", id="remove-trustline")
                    if self.domain:
                        yield Button("Discover anchor [A]", id="discover-anchor")
                        yield Button("Deposit [D]", id="anchor-deposit")
                        yield Button("Withdraw [W]", id="anchor-withdraw")
                yield Button("Back [Esc]", id="close")
        yield Footer()

    def on_mount(self) -> None:
        if self.runtime is None:
            self.runtime = getattr(self.app, "runtime", None)
        if self.on_trustline_action is None:
            self.on_trustline_action = getattr(self.app, "_on_trustline_action", None)
        self._render_balance()
        for selector in ("#anchor-deposit", "#anchor-withdraw"):
            buttons = self.query(selector)
            if buttons:
                buttons.first().display = False

    def action_close(self) -> None:
        self.dismiss(None)

    def action_send(self) -> None:
        if not self.asset.is_liquidity_pool:
            self.dismiss(AssetDetailAction("send", _asset_identity(self.balance)))

    def action_set_limit(self) -> None:
        if self.asset.is_native or self.asset.is_liquidity_pool:
            return
        self.app.push_screen(
            TrustlineFormDialog(
                "limit",
                asset=_asset_identity(self.balance),
                limit=str(self.balance.raw.get("limit", "")),
            ),
            self._on_trustline_form,
        )

    def action_remove_trustline(self) -> None:
        if self.asset.is_native or self.asset.is_liquidity_pool:
            return
        if self.on_trustline_action is None:
            self.set_status("Trustline actions are unavailable in this runtime.")
            return
        self.on_trustline_action(
            self,
            TrustlineAction(kind="remove", asset=_asset_identity(self.balance)),
        )

    def _on_trustline_form(self, action: TrustlineAction | None) -> None:
        if action is not None and self.on_trustline_action is not None:
            self.on_trustline_action(self, action)

    def action_anchor(self) -> None:
        if (
            not self.domain
            or self.asset.is_native
            or self.asset.is_liquidity_pool
            or self._anchor_loading
        ):
            return
        self._anchor_loading = True
        self.query_one("#asset-anchor", Static).update(
            f"Anchor discovery: loading stellar.toml and transfer capabilities from {self.domain}..."
        )
        self._discover_anchor(self.domain)

    def action_anchor_deposit(self) -> None:
        self._start_anchor("deposit")

    def action_anchor_withdraw(self) -> None:
        self._start_anchor("withdraw")

    def _start_anchor(self, kind: Literal["deposit", "withdraw"]) -> None:
        capabilities = self._anchor_capabilities
        if capabilities is None:
            self.set_status("Discover anchor capabilities first [A].")
            return
        enabled = capabilities.sep24_deposit if kind == "deposit" else capabilities.sep24_withdraw
        if not enabled:
            self.set_status(f"SEP-24 {kind} is not available for this asset.")
            return
        if self.runtime is None:
            self.set_status("Anchor transfers are unavailable in this runtime.")
            return
        try:
            record = self.runtime.wallet_manager.get_record()
            state = self.runtime.wallet_manager.state(record.name)
        except FresnicaError as exc:
            self.set_status(str(exc))
            return
        if state is WalletState.WATCH_ONLY:
            self.set_status("Watch-only wallet cannot sign SEP-10 authentication.")
            return
        if state is WalletState.LOCKED:
            self.app.push_screen(
                UnlockDialog(record.name),
                lambda password: self._after_anchor_unlock(kind, record.name, password),
            )
            return
        self._begin_anchor_transfer(kind)

    def _after_anchor_unlock(self, kind: str, wallet_name: str, password: str | None) -> None:
        if password is None or self.runtime is None:
            return
        try:
            self.runtime.wallet_manager.unlock(wallet_name, password)
        except (FresnicaError, ValueError) as exc:
            self.app.push_screen(
                UnlockDialog(wallet_name, error=str(exc)),
                lambda retry: self._after_anchor_unlock(kind, wallet_name, retry),
            )
            return
        self._begin_anchor_transfer(kind)

    def _begin_anchor_transfer(self, kind: str) -> None:
        self.set_status(f"Authenticating with anchor for {kind}...")
        self._run_anchor_transfer(kind)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _run_anchor_transfer(self, kind: str) -> None:
        try:
            if self.runtime is None or self._anchor_capabilities is None:
                raise ValueError("Anchor transfer context is unavailable")
            session = self.runtime.wallet_manager.current()
            if session is None:
                raise WalletLockedError("Wallet is locked")
            network = get_network(session.record.network)
            transfer = AnchorService().start_sep24(
                session.wallet,
                self.asset,
                self._anchor_capabilities,
                kind,
                network.passphrase,
            )
            self.app.call_from_thread(self._finish_anchor_transfer, transfer, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._finish_anchor_transfer, None, exc)

    def _finish_anchor_transfer(self, transfer, error) -> None:
        if not self.is_mounted:
            return
        if error is not None:
            self.set_status(f"Anchor transfer failed: {error}")
            return
        assert transfer is not None
        opened = bool(webbrowser.open(transfer.url, new=2))
        self.query_one("#asset-anchor", Static).update(
            f"Anchor {transfer.kind} session:\n{transfer.url}"
        )
        if opened:
            self.set_status(f"Opened anchor {transfer.kind} flow in the system browser.")
        else:
            self.set_status("Browser did not open automatically · use the URL shown above.")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        actions = {
            "close": self.action_close,
            "send": self.action_send,
            "set-limit": self.action_set_limit,
            "remove-trustline": self.action_remove_trustline,
            "discover-anchor": self.action_anchor,
            "anchor-deposit": self.action_anchor_deposit,
            "anchor-withdraw": self.action_anchor_withdraw,
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
        self._anchor_loading = False
        if not self.is_mounted:
            return
        widget = self.query_one("#asset-anchor", Static)
        if error is not None:
            widget.update(f"Anchor discovery: unavailable ({error})")
            return
        if capabilities is None:
            return
        self._anchor_capabilities = capabilities
        parts = [f"Anchor: {capabilities.domain}"]
        if capabilities.sep24_url:
            methods = _methods(capabilities.sep24_deposit, capabilities.sep24_withdraw)
            parts.append(f"Interactive transfer (SEP-24): {methods}")
        if capabilities.sep6_url:
            methods = _methods(capabilities.sep6_deposit, capabilities.sep6_withdraw)
            parts.append(f"Programmatic SEP-6: {methods}")
            if not capabilities.sep24_url:
                parts.append("SEP-6-only KYC flow is not exposed as a partial wallet action")
        if capabilities.web_auth_url:
            parts.append("SEP-10 authentication: available")
        parts.extend(f"Note: {warning}" for warning in capabilities.warnings)
        if len(parts) == 1:
            parts.append("No SEP-6/SEP-24 transfer service advertised")
        widget.update("\n".join(parts))

        deposit = self.query("#anchor-deposit")
        withdraw = self.query("#anchor-withdraw")
        if deposit:
            deposit.first().display = bool(
                capabilities.sep24_deposit
                and capabilities.web_auth_url
                and capabilities.signing_key
            )
        if withdraw:
            withdraw.first().display = bool(
                capabilities.sep24_withdraw
                and capabilities.web_auth_url
                and capabilities.signing_key
            )

    def refresh_trustlines(self) -> None:
        """Refresh only this asset after the shared trustline pipeline submits."""
        if self.runtime is None or self.asset.is_native or self.asset.is_liquidity_pool:
            return
        self.set_status("Refreshing this asset trustline...")
        self._refresh_asset()

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _refresh_asset(self) -> None:
        try:
            session = self.runtime.wallet_manager.view()
            service = self.runtime.services_for(session.record.network).balance_service
            balances, _ = service.get_portfolio_views(session.wallet)
            match = next((item for item in balances if item.asset == self.asset), None)
            self.app.call_from_thread(self._apply_asset_refresh, match, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply_asset_refresh, None, exc)

    def _apply_asset_refresh(self, balance: BalanceView | None, error) -> None:
        if not self.is_mounted:
            return
        if error is not None:
            self.set_status(f"Unable to refresh asset: {error}")
            return
        if balance is None:
            self.query_one("#asset-trustline", Static).update("Trustline removed")
            self.set_status("Trustline removed · return to Assets [Esc].")
            for selector in ("#set-limit", "#remove-trustline"):
                widgets = self.query(selector)
                if widgets:
                    widgets.first().display = False
            return
        self.balance = balance
        self.domain = str(balance.raw.get(ISSUER_DOMAIN_CACHE_KEY) or self.domain).strip()
        self._render_balance()
        self.set_status("Asset trustline updated.")

    def _render_balance(self) -> None:
        source = asset_source(self.asset, self.domain or None)
        self.query_one("#asset-identity", Static).update(_asset_identity(self.balance))
        in_offers = (
            ""
            if self.balance.selling_liabilities == 0
            else f"\nIn offers: {format_amount(self.balance.selling_liabilities)}"
        )
        self.query_one("#asset-balance", Static).update(
            f"Source: {source}\n"
            f"Balance: {format_amount(self.balance.balance)}\n"
            f"Available: {format_amount(self.balance.available)}"
            f"{in_offers}"
        )
        self.query_one("#asset-trustline", Static).update(_trustline_text(self.balance))

    def set_status(self, message: str) -> None:
        if self.is_mounted:
            self.query_one("#asset-status", Static).update(message)


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
    if domain:
        return f"Anchor metadata available from {domain} · Discover [A] only when you want transfer services"
    return "Anchor discovery: issuer has no home_domain"


def _methods(deposit: bool, withdraw: bool) -> str:
    methods = []
    if deposit:
        methods.append("deposit")
    if withdraw:
        methods.append("withdraw")
    return "/".join(methods) if methods else "asset methods unavailable"

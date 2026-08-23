"""Asset-level wallet actions and explicit anchor transfer presentation."""

from dataclasses import dataclass
from typing import Literal
import webbrowser

from rich.text import Text
from textual import work
from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Button, Footer, Input, Label, Static

from ..anchor_service import AnchorCapabilities
from ..anchor_transfer_service import (
    AnchorDepositInstructions,
    AnchorKycRequired,
    AnchorNeedFields,
    AnchorOpenUrl,
    AnchorTransferPlan,
    AnchorWithdrawalPayment,
)
from ..balance_service import ISSUER_DOMAIN_CACHE_KEY
from ..errors import FresnicaError
from ..manager import WalletState
from ..models import BalanceView
from ..network import get_network
from ..presentation import asset_source, format_amount, short_address
from .screens import SendDialog, UnlockDialog
from .trustlines import TrustlineAction


AssetDetailActionKind = Literal["send"]
# Compatibility for app.py while the presentation-specific name ages out.
AnchorWithdrawalRequest = AnchorWithdrawalPayment


@dataclass(frozen=True)
class AssetDetailAction:
    kind: AssetDetailActionKind
    asset: str


class Sep6TransferDialog(ModalScreen[dict | None]):
    BINDINGS = [("escape", "cancel", "Cancel")]

    CSS = """
    Sep6TransferDialog { align: center middle; }
    Sep6TransferDialog > #dialog { width: 94; height: auto; max-height: 90%; padding: 1 2; border: round $accent; background: $surface; }
    Sep6TransferDialog Input { margin-top: 1; }
    Sep6TransferDialog .field-help { color: $text-muted; height: auto; }
    Sep6TransferDialog #form-error { color: $error; height: auto; margin-top: 1; }
    Sep6TransferDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }
    Sep6TransferDialog Button { margin-left: 1; }
    """

    def __init__(self, kind: str, asset_code: str, plan: AnchorTransferPlan):
        super().__init__()
        self.kind = kind
        self.asset_code = asset_code
        self.transfer_type = plan.transfer_type
        self.fields = [
            (name, spec if isinstance(spec, dict) else {})
            for name, spec in plan.user_fields.items()
        ]

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label(f"SEP-6 {self.kind} · {self.asset_code}")
            if self.transfer_type:
                yield Static(f"Method: {self.transfer_type}", classes="field-help")
            for index, (name, spec) in enumerate(self.fields):
                optional = bool(spec.get("optional", False))
                marker = "optional" if optional else "required"
                description = str(spec.get("description") or "").strip()
                yield Label(f"{name} ({marker})")
                if description:
                    yield Static(description, classes="field-help")
                yield Input(placeholder=name, id=f"sep6-field-{index}")
            yield Static("", id="form-error")
            with Horizontal(id="actions"):
                yield Button("Cancel [Esc]", id="cancel")
                yield Button("Continue", id="continue", variant="primary")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "cancel":
            self.action_cancel()
            return
        if event.button.id != "continue":
            return
        result = {}
        if self.transfer_type:
            result["type"] = self.transfer_type
        for index, (name, spec) in enumerate(self.fields):
            value = self.query_one(f"#sep6-field-{index}", Input).value.strip()
            if not value and not bool(spec.get("optional", False)):
                self.query_one("#form-error", Static).update(f"{name} is required.")
                return
            if value:
                result[name] = value
        self.dismiss(result)


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
                yield Button(Text("Cancel [Esc]"), id="cancel")
                yield Button("Review", id="review", variant="primary")


class AssetDetailsScreen(ModalScreen[AssetDetailAction | None]):
    BINDINGS = [
        Binding("escape", "close", "Back"),
        Binding("s", "send", "Send"),
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
                    yield Button(Text("Send [S]"), id="send", variant="primary")
                if not self.asset.is_native and not self.asset.is_liquidity_pool:
                    yield Button(Text("Remove [X]"), id="remove-trustline")
                    if self.domain:
                        yield Button(Text("Discover anchor [A]"), id="discover-anchor")
                        yield Button(Text("Deposit [D]"), id="anchor-deposit")
                        yield Button(Text("Withdraw [W]"), id="anchor-withdraw")
                yield Button(Text("Back [Esc]"), id="close")
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
        self._load_cached_anchor()

    def action_close(self) -> None:
        self.dismiss(None)

    def action_send(self) -> None:
        if not self.asset.is_liquidity_pool:
            self.dismiss(AssetDetailAction("send", _asset_identity(self.balance)))

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
        service = getattr(self.runtime, "anchor_transfer_service", None) if self.runtime else None
        if capabilities is None:
            self.set_status("Discover anchor capabilities first (A).")
            return
        if service is None:
            self.set_status("Anchor transfer workflow is unavailable in this runtime.")
            return
        try:
            plan = service.plan(capabilities, kind)
        except (FresnicaError, ValueError) as exc:
            self.set_status(str(exc))
            return
        if plan.requires_fields:
            self.app.push_screen(
                Sep6TransferDialog(kind, self.asset.code, plan),
                lambda values: self._begin_anchor_transfer(plan, values),
            )
            return
        self._begin_anchor_transfer(plan, {})

    def _begin_anchor_transfer(
        self,
        plan: AnchorTransferPlan,
        fields: dict | None,
    ) -> None:
        if fields is None or self.runtime is None:
            return
        try:
            record = self.runtime.wallet_manager.get_record()
            state = self.runtime.wallet_manager.state(record.name)
        except FresnicaError as exc:
            self.set_status(str(exc))
            return
        if plan.requires_signing and state is WalletState.WATCH_ONLY:
            self.set_status("Watch-only wallet cannot complete this anchor transfer.")
            return
        if plan.requires_signing and state is WalletState.LOCKED:
            self.app.push_screen(
                UnlockDialog(record.name),
                lambda password: self._after_anchor_unlock(
                    plan, fields, record.name, password
                ),
            )
            return
        session = (
            self.runtime.wallet_manager.current()
            if plan.requires_signing
            else self.runtime.wallet_manager.view()
        )
        if session is None:
            self.set_status("Wallet is locked.")
            return
        self.set_status(f"Starting anchor {plan.kind} via {plan.protocol.upper()}...")
        self._run_anchor_transfer(plan, fields, session.wallet)

    def _after_anchor_unlock(
        self,
        plan: AnchorTransferPlan,
        fields: dict,
        wallet_name: str,
        password: str | None,
    ) -> None:
        if password is None or self.runtime is None:
            return
        try:
            self.runtime.wallet_manager.unlock(wallet_name, password)
        except (FresnicaError, ValueError) as exc:
            self.app.push_screen(
                UnlockDialog(wallet_name, error=str(exc)),
                lambda retry: self._after_anchor_unlock(
                    plan, fields, wallet_name, retry
                ),
            )
            return
        self._begin_anchor_transfer(plan, fields)

    @work(exclusive=True, thread=True, exit_on_error=False)
    def _run_anchor_transfer(self, plan: AnchorTransferPlan, fields: dict, wallet) -> None:
        try:
            if self.runtime is None or self._anchor_capabilities is None:
                raise ValueError("Anchor transfer context is unavailable")
            service = getattr(self.runtime, "anchor_transfer_service", None)
            if service is None:
                raise ValueError("Anchor transfer workflow is unavailable")
            record = self.runtime.wallet_manager.get_record()
            network = get_network(record.network)
            outcome = service.start(
                wallet,
                self.asset,
                self._anchor_capabilities,
                plan.kind,
                network.passphrase,
                fields=fields,
                plan=plan,
            )
            self.app.call_from_thread(self._finish_anchor_transfer, outcome, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._finish_anchor_transfer, None, exc)

    def _finish_anchor_transfer(self, outcome, error) -> None:
        if not self.is_mounted:
            return
        if error is not None:
            self.set_status(f"Anchor transfer failed: {error}")
            return
        if isinstance(outcome, AnchorNeedFields):
            self.app.push_screen(
                Sep6TransferDialog(outcome.plan.kind, self.asset.code, outcome.plan),
                lambda values: self._begin_anchor_transfer(outcome.plan, values),
            )
            return
        if isinstance(outcome, AnchorOpenUrl):
            opened = bool(webbrowser.open(outcome.url, new=2))
            self.query_one("#asset-anchor", Static).update(
                f"Anchor {outcome.kind} session:\n{outcome.url}"
            )
            if opened:
                self.set_status(
                    f"Opened anchor {outcome.kind} flow in the system browser."
                )
            else:
                self.set_status(
                    "Browser did not open automatically · use the URL shown above."
                )
            return
        if isinstance(outcome, AnchorKycRequired):
            self.query_one("#asset-anchor", Static).update(
                _anchor_payload_text(outcome.kind, outcome.payload)
            )
            self.set_status(
                "Anchor requires customer information · SEP-12/KYC handoff is not exposed yet."
            )
            return
        if isinstance(outcome, AnchorDepositInstructions):
            self.query_one("#asset-anchor", Static).update(
                _anchor_payload_text("deposit", outcome.payload)
            )
            self.set_status("SEP-6 deposit instructions ready.")
            return
        if isinstance(outcome, AnchorWithdrawalPayment):
            self.query_one("#asset-anchor", Static).update(
                _anchor_payload_text("withdraw", outcome.payload)
            )
            handler = getattr(self.app, "prepare_anchor_withdrawal", None)
            if handler is None:
                self.set_status("Anchor withdrawal payment pipeline is unavailable.")
                return
            handler(self, outcome)
            return
        self.set_status("Anchor returned an unsupported transfer outcome.")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        actions = {
            "close": self.action_close,
            "send": self.action_send,
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
            if self.runtime is None:
                raise ValueError("Anchor transfer context is unavailable")
            service = getattr(self.runtime, "anchor_transfer_service", None)
            if service is None:
                raise ValueError("Anchor transfer workflow is unavailable")
            capabilities = service.discover(self.asset, domain)
            self.app.call_from_thread(self._apply_anchor, capabilities, None)
        except (FresnicaError, ValueError) as exc:
            self.app.call_from_thread(self._apply_anchor, None, exc)

    def _load_cached_anchor(self) -> None:
        if self.runtime is None or not self.domain:
            return
        store = getattr(self.runtime, "anchor_capabilities_store", None)
        if store is None:
            return
        try:
            record = self.runtime.wallet_manager.get_record()
            capabilities = store.get(record.network, self.asset, self.domain)
        except FresnicaError:
            return
        if capabilities is not None:
            self._show_anchor(capabilities)

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
        cache_error = None
        store = getattr(self.runtime, "anchor_capabilities_store", None) if self.runtime else None
        if store is not None:
            try:
                record = self.runtime.wallet_manager.get_record()
                store.put(record.network, self.asset, capabilities)
            except FresnicaError as exc:
                cache_error = exc
        self._show_anchor(capabilities)
        if cache_error is not None:
            self.set_status(f"Anchor discovered · cache unavailable: {cache_error}")
        else:
            self.set_status("Anchor capabilities updated and cached.")

    def _show_anchor(self, capabilities: AnchorCapabilities) -> None:
        self._anchor_capabilities = capabilities
        widget = self.query_one("#asset-anchor", Static)
        parts = [f"Anchor: {capabilities.domain}"]
        if capabilities.sep24_url:
            methods = _methods(capabilities.sep24_deposit, capabilities.sep24_withdraw)
            parts.append(f"Interactive transfer (SEP-24): {methods}")
        if capabilities.sep6_url:
            methods = _methods(capabilities.sep6_deposit, capabilities.sep6_withdraw)
            parts.append(f"Programmatic SEP-6: {methods}")
        if capabilities.web_auth_url:
            parts.append("SEP-10 authentication: available")
        parts.extend(f"Note: {warning}" for warning in capabilities.warnings)
        if len(parts) == 1:
            parts.append("No SEP-6/SEP-24 transfer service advertised")
        widget.update("\n".join(parts))

        discover = self.query("#discover-anchor")
        if discover:
            discover.first().label = Text("Refresh anchor [A]")
        deposit = self.query("#anchor-deposit")
        withdraw = self.query("#anchor-withdraw")
        if deposit:
            deposit.first().display = bool(
                (
                    capabilities.sep24_deposit
                    and capabilities.web_auth_url
                    and capabilities.signing_key
                )
                or capabilities.sep6_deposit
            )
        if withdraw:
            withdraw.first().display = bool(
                (
                    capabilities.sep24_withdraw
                    and capabilities.web_auth_url
                    and capabilities.signing_key
                )
                or capabilities.sep6_withdraw
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
            self.set_status("Trustline removed · return to Assets (Esc).")
            for selector in ("#remove-trustline",):
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


def _anchor_payload_text(kind: str, payload: dict) -> str:
    lines = [f"SEP-6 {kind}:"]
    how = _optional_payload_text(payload.get("how"))
    if how:
        lines.append(how)
    for key, label in (
        ("account_id", "Stellar account"),
        ("memo_type", "Memo type"),
        ("memo", "Memo"),
        ("fee_fixed", "Fixed fee"),
        ("fee_percent", "Fee percent"),
        ("min_amount", "Minimum"),
    ):
        value = payload.get(key)
        if value is not None and str(value).strip():
            lines.append(f"{label}: {value}")
    extra = payload.get("extra_info")
    if isinstance(extra, dict):
        extra = extra.get("message")
    extra_text = _optional_payload_text(extra)
    if extra_text:
        lines.append(extra_text)
    if len(lines) == 1:
        lines.append(str(payload))
    return "\n".join(lines)


def _optional_payload_text(value) -> str | None:
    return str(value).strip() if value is not None and str(value).strip() else None


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
        return f"Anchor metadata available from {domain} · press A to discover transfer services"
    return "Anchor discovery: issuer has no home_domain"


def _methods(deposit: bool, withdraw: bool) -> str:
    methods = []
    if deposit:
        methods.append("deposit")
    if withdraw:
        methods.append("withdraw")
    return "/".join(methods) if methods else "asset methods unavailable"

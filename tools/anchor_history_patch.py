from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / "reference" / "python"


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"pattern not found in {path}: {old[:160]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_between(path: Path, start: str, end: str, replacement: str) -> None:
    text = path.read_text(encoding="utf-8")
    a = text.find(start)
    if a < 0:
        raise SystemExit(f"start marker not found in {path}: {start[:120]!r}")
    b = text.find(end, a)
    if b < 0:
        raise SystemExit(f"end marker not found in {path}: {end[:120]!r}")
    path.write_text(text[:a] + replacement + text[b:], encoding="utf-8")


# ---------------------------------------------------------------------------
# Anchor workflow boundary: protocol transport stays in AnchorService; product
# protocol selection, field planning, and response interpretation live here.
# ---------------------------------------------------------------------------
anchor_transfer = PY / "fresnica" / "anchor_transfer_service.py"
anchor_transfer.write_text('''"""Wallet-level anchor transfer workflow independent from presentation layers."""

from dataclasses import dataclass, field
from typing import Literal, TypeAlias

from .anchor_service import AnchorCapabilities, AnchorError, AnchorService
from .models import Asset


AnchorTransferKind = Literal["deposit", "withdraw"]
AnchorProtocol = Literal["sep24", "sep6"]


@dataclass(frozen=True)
class AnchorTransferPlan:
    kind: AnchorTransferKind
    protocol: AnchorProtocol
    requires_signing: bool
    transfer_type: str | None = None
    fields: dict = field(default_factory=dict)

    @property
    def user_fields(self) -> dict:
        return {
            name: spec
            for name, spec in self.fields.items()
            if name not in {"asset_code", "account"}
        }

    @property
    def requires_fields(self) -> bool:
        return bool(self.user_fields)


@dataclass(frozen=True)
class AnchorNeedFields:
    plan: AnchorTransferPlan


@dataclass(frozen=True)
class AnchorOpenUrl:
    kind: AnchorTransferKind
    url: str
    transaction_id: str | None = None


@dataclass(frozen=True)
class AnchorKycRequired:
    kind: AnchorTransferKind
    payload: dict


@dataclass(frozen=True)
class AnchorDepositInstructions:
    payload: dict


@dataclass(frozen=True)
class AnchorWithdrawalPayment:
    asset: Asset
    amount: str
    destination: str
    memo: str | None = None
    memo_type: str | None = None
    anchor_domain: str | None = None
    extra_info: str | None = None
    payload: dict = field(default_factory=dict)


AnchorTransferOutcome: TypeAlias = (
    AnchorNeedFields
    | AnchorOpenUrl
    | AnchorKycRequired
    | AnchorDepositInstructions
    | AnchorWithdrawalPayment
)


class AnchorTransferService:
    """Translate discovered anchor capabilities into wallet-level next actions."""

    def __init__(self, protocol_service: AnchorService | None = None):
        self.protocol_service = protocol_service or AnchorService()

    def discover(self, asset: Asset, home_domain: str) -> AnchorCapabilities:
        return self.protocol_service.discover(asset, home_domain)

    def plan(
        self,
        capabilities: AnchorCapabilities,
        kind: AnchorTransferKind,
    ) -> AnchorTransferPlan:
        if kind not in {"deposit", "withdraw"}:
            raise ValueError(f"Unsupported anchor transfer kind: {kind}")

        sep24_enabled = (
            capabilities.sep24_deposit if kind == "deposit" else capabilities.sep24_withdraw
        )
        if (
            sep24_enabled
            and capabilities.sep24_url
            and capabilities.web_auth_url
            and capabilities.signing_key
        ):
            return AnchorTransferPlan(kind=kind, protocol="sep24", requires_signing=True)

        sep6_enabled = (
            capabilities.sep6_deposit if kind == "deposit" else capabilities.sep6_withdraw
        )
        if sep6_enabled and capabilities.sep6_url:
            info = (
                capabilities.sep6_deposit_info
                if kind == "deposit"
                else capabilities.sep6_withdraw_info
            )
            transfer_type, fields = _sep6_schema(info)
            requires_signing = kind == "withdraw" or bool(
                info.get("authentication_required", False)
                if isinstance(info, dict)
                else False
            )
            return AnchorTransferPlan(
                kind=kind,
                protocol="sep6",
                requires_signing=requires_signing,
                transfer_type=transfer_type,
                fields=fields,
            )

        raise AnchorError(
            f"No usable SEP-24/SEP-6 {kind} flow is advertised for this asset"
        )

    def start(
        self,
        wallet,
        asset: Asset,
        capabilities: AnchorCapabilities,
        kind: AnchorTransferKind,
        network_passphrase: str,
        *,
        fields: dict | None = None,
        plan: AnchorTransferPlan | None = None,
    ) -> AnchorTransferOutcome:
        plan = plan or self.plan(capabilities, kind)
        if plan.kind != kind:
            raise AnchorError("Anchor transfer plan does not match requested action")
        if plan.requires_signing and not wallet.can_sign():
            raise AnchorError("This anchor transfer requires a signing wallet")

        if plan.protocol == "sep24":
            transfer = self.protocol_service.start_sep24(
                wallet,
                asset,
                capabilities,
                kind,
                network_passphrase,
            )
            return AnchorOpenUrl(
                kind=kind,
                url=transfer.url,
                transaction_id=transfer.transaction_id,
            )

        if plan.requires_fields and fields is None:
            return AnchorNeedFields(plan)

        request_fields = dict(fields or {})
        _validate_fields(plan, request_fields)
        if plan.transfer_type and "type" not in request_fields:
            request_fields["type"] = plan.transfer_type
        transfer = self.protocol_service.start_sep6(
            wallet,
            asset,
            capabilities,
            kind,
            network_passphrase,
            request_fields,
        )
        return _interpret_sep6(asset, capabilities, transfer)


def _interpret_sep6(asset, capabilities, transfer) -> AnchorTransferOutcome:
    payload = dict(transfer.payload)
    response_type = str(payload.get("type") or "")
    if response_type in {
        "non_interactive_customer_info_needed",
        "customer_info_status",
    }:
        return AnchorKycRequired(kind=transfer.kind, payload=payload)

    if transfer.kind == "deposit":
        return AnchorDepositInstructions(payload=payload)

    destination = _optional_text(payload.get("account_id"))
    amount = _optional_text(transfer.request.get("amount"))
    if not destination or not amount:
        raise AnchorError(
            "SEP-6 withdraw response is missing account_id or requested amount"
        )
    extra = payload.get("extra_info")
    if isinstance(extra, dict):
        extra = extra.get("message")
    return AnchorWithdrawalPayment(
        asset=asset,
        amount=amount,
        destination=destination,
        memo=_optional_text(payload.get("memo")),
        memo_type=_optional_text(payload.get("memo_type")),
        anchor_domain=capabilities.domain,
        extra_info=_optional_text(extra),
        payload=payload,
    )


def _validate_fields(plan: AnchorTransferPlan, values: dict) -> None:
    missing = []
    for name, raw_spec in plan.user_fields.items():
        spec = raw_spec if isinstance(raw_spec, dict) else {}
        if spec.get("optional", False):
            continue
        if not str(values.get(name) or "").strip():
            missing.append(name)
    if missing:
        raise AnchorError(f"Anchor transfer requires field: {missing[0]}")


def _sep6_schema(info: dict) -> tuple[str | None, dict]:
    if not isinstance(info, dict):
        return None, {}
    types = info.get("types")
    if isinstance(types, dict) and types:
        # A single advertised method can be selected without presentation-layer
        # protocol knowledge. Multiple methods need an explicit future chooser.
        if len(types) != 1:
            raise AnchorError("Anchor advertises multiple SEP-6 transfer methods")
        transfer_type = next(iter(types))
        spec = types.get(transfer_type)
        fields = spec.get("fields", {}) if isinstance(spec, dict) else {}
        return transfer_type, dict(fields) if isinstance(fields, dict) else {}
    fields = info.get("fields", {})
    return None, dict(fields) if isinstance(fields, dict) else {}


def _optional_text(value) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None
''', encoding="utf-8")

sync_module = PY / "fresnica" / "sync.py"
sync_module.write_text('''"""Small shared result contract for bounded chain synchronization."""

from dataclasses import dataclass


@dataclass(frozen=True)
class SyncResult:
    fetched_count: int
    caught_up: bool
''', encoding="utf-8")

# Runtime owns one presentation-independent anchor transfer orchestrator.
runtime = PY / "fresnica" / "runtime.py"
replace_once(
    runtime,
    "from .anchor_cache import AnchorCapabilitiesStore\n",
    "from .anchor_cache import AnchorCapabilitiesStore\nfrom .anchor_transfer_service import AnchorTransferService\n",
)
replace_once(
    runtime,
    "        self.anchor_capabilities_store = AnchorCapabilitiesStore(self.home / \"anchors.json\")\n        self.wallet_manager = WalletManager(self.wallet_storage)\n",
    "        self.anchor_capabilities_store = AnchorCapabilitiesStore(self.home / \"anchors.json\")\n        self.anchor_transfer_service = AnchorTransferService()\n        self.wallet_manager = WalletManager(self.wallet_storage)\n",
)

# Anchor discovery cache must be network-scoped just like chain-derived caches.
cache = PY / "fresnica" / "anchor_cache.py"
replace_once(
    cache,
    "    def get(self, asset: Asset, domain: str) -> AnchorCapabilities | None:\n        key = _key(asset, domain)\n",
    "    def get(self, network: str, asset: Asset, domain: str) -> AnchorCapabilities | None:\n        key = _key(network, asset, domain)\n",
)
replace_once(
    cache,
    "    def put(self, asset: Asset, capabilities: AnchorCapabilities) -> None:\n        key = _key(asset, capabilities.domain)\n",
    "    def put(self, network: str, asset: Asset, capabilities: AnchorCapabilities) -> None:\n        key = _key(network, asset, capabilities.domain)\n",
)
replace_once(
    cache,
    "def _key(asset: Asset, domain: str) -> str | None:\n    if asset.is_native or asset.is_liquidity_pool or not asset.issuer:\n        return None\n    host = str(domain or \"\").strip().lower().rstrip(\".\")\n    if not host:\n        return None\n    return f\"{asset.code}:{asset.issuer}@{host}\"\n",
    "def _key(network: str, asset: Asset, domain: str) -> str | None:\n    if asset.is_native or asset.is_liquidity_pool or not asset.issuer:\n        return None\n    network_name = str(network or \"\").strip().lower()\n    host = str(domain or \"\").strip().lower().rstrip(\".\")\n    if not network_name or not host:\n        return None\n    return f\"{network_name}|{asset.code}:{asset.issuer}@{host}\"\n",
)

# ---------------------------------------------------------------------------
# History bounded catch-up.
# ---------------------------------------------------------------------------
history = PY / "fresnica" / "history_service.py"
replace_once(
    history,
    "from .presentation import format_amount, short_address\n",
    "from .presentation import format_amount, short_address\nfrom .sync import SyncResult\n",
)
replace_once(
    history,
    "SYNC_PAGE_LIMIT = 200\n",
    "SYNC_PAGE_LIMIT = 200\nSYNC_MAX_INCREMENTAL_PAGES = 5\n",
)
replace_between(
    history,
    "    def sync_recent(self, wallet, limit: int = SYNC_PAGE_LIMIT) -> int:\n",
    "    def load_older(self, wallet, limit: int = SYNC_PAGE_LIMIT) -> int:\n",
'''    def sync_recent(self, wallet, limit: int = SYNC_PAGE_LIMIT) -> SyncResult:\n        """Catch up newer operations with a bounded forward pagination loop."""\n        address = wallet.address()\n        cached = self.datastore.get_operations(self.network_name, address, limit=1)\n        if not cached:\n            response = self.adapter.get_operations(address, limit=limit, desc=True)\n            records = list(response.get("_embedded", {}).get("records", []))\n            if records:\n                self.datastore.save_operations(self.network_name, address, response)\n            # A descending first page is anchored at Horizon head even when older\n            # history exists; older data is explicitly fetched via load_older().\n            return SyncResult(fetched_count=len(records), caught_up=True)\n\n        next_cursor = _paging_token(cached[0])\n        fetched = 0\n        for _ in range(SYNC_MAX_INCREMENTAL_PAGES):\n            response = self.adapter.get_operations(\n                address,\n                limit=limit,\n                cursor=next_cursor,\n                desc=False,\n            )\n            records = list(response.get("_embedded", {}).get("records", []))\n            if not records:\n                return SyncResult(fetched_count=fetched, caught_up=True)\n            self.datastore.save_operations(self.network_name, address, response)\n            fetched += len(records)\n            last_cursor = _paging_token(records[-1])\n            if not last_cursor or last_cursor == next_cursor:\n                return SyncResult(fetched_count=fetched, caught_up=False)\n            next_cursor = last_cursor\n            if len(records) < limit:\n                return SyncResult(fetched_count=fetched, caught_up=True)\n        return SyncResult(fetched_count=fetched, caught_up=False)\n\n'''
)
replace_once(
    history,
    "        if not cached:\n            return self.sync_recent(wallet, limit=limit)\n",
    "        if not cached:\n            return self.sync_recent(wallet, limit=limit).fetched_count\n",
)

# History full-screen consumes the explicit catch-up result.
history_ui = PY / "fresnica" / "tui" / "history.py"
replace_once(
    history_ui,
    "            if sync_recent is not None:\n                sync_recent(self.wallet)\n                views = self.history_service.get_activity_views(\n                    self.wallet,\n                    limit=100000,\n                    refresh=False,\n                )\n            else:\n                views = self.history_service.get_activity_views(\n                    self.wallet,\n                    limit=100000,\n                    refresh=True,\n                )\n            count = self._cached_operation_count(views)\n            self.app.call_from_thread(self._apply, views, count, \"Activity updated\", None)\n",
    "            sync_result = None\n            if sync_recent is not None:\n                sync_result = sync_recent(self.wallet)\n                views = self.history_service.get_activity_views(\n                    self.wallet,\n                    limit=100000,\n                    refresh=False,\n                )\n            else:\n                views = self.history_service.get_activity_views(\n                    self.wallet,\n                    limit=100000,\n                    refresh=True,\n                )\n            count = self._cached_operation_count(views)\n            caught_up = bool(getattr(sync_result, \"caught_up\", True))\n            fetched = int(getattr(sync_result, \"fetched_count\", 0))\n            message = (\n                \"Activity updated\"\n                if caught_up\n                else f\"Activity catch-up incomplete · {fetched} newer operations cached · refresh again\"\n            )\n            self.app.call_from_thread(self._apply, views, count, message, None)\n",
)

# Dashboard refresh must not claim full freshness while bounded history catch-up
# still has pages remaining.
app_base = PY / "fresnica" / "tui" / "app_base.py"
replace_once(
    app_base,
    "            balances, positions = services.balance_service.get_portfolio_views(session.wallet)\n            history = services.history_service.get_views(session.wallet, limit=20)\n            self.call_from_thread(\n                self._apply_wallet,\n                record,\n                balances,\n                positions,\n                history,\n                ready_message,\n                None,\n            )\n",
    "            balances, positions = services.balance_service.get_portfolio_views(session.wallet)\n            history_sync = None\n            sync_recent = getattr(services.history_service, \"sync_recent\", None)\n            if sync_recent is not None:\n                history_sync = sync_recent(session.wallet)\n                history = services.history_service.get_views(\n                    session.wallet, limit=20, refresh=False\n                )\n            else:\n                history = services.history_service.get_views(session.wallet, limit=20)\n            self.call_from_thread(\n                self._apply_wallet,\n                record,\n                balances,\n                positions,\n                history,\n                ready_message,\n                None,\n                bool(getattr(history_sync, \"caught_up\", True)),\n            )\n",
)
replace_once(
    app_base,
    "    def _apply_wallet(self, record, balances, positions, history, ready_message, error) -> None:\n",
    "    def _apply_wallet(\n        self,\n        record,\n        balances,\n        positions,\n        history,\n        ready_message,\n        error,\n        history_caught_up: bool = True,\n    ) -> None:\n",
)
replace_once(
    app_base,
    "        self._set_sync(f\"Updated {datetime.now().strftime('%H:%M:%S')}\")\n",
    "        updated = f\"Updated {datetime.now().strftime('%H:%M:%S')}\"\n        if not history_caught_up:\n            updated += \" · activity catch-up incomplete; refresh again\"\n        self._set_sync(updated)\n",
)

# ---------------------------------------------------------------------------
# TUI anchor screen: consume workflow plans/outcomes; no SEP selection or
# response interpretation remains in presentation code.
# ---------------------------------------------------------------------------
asset_details = PY / "fresnica" / "tui" / "asset_details.py"
replace_once(
    asset_details,
    "from ..anchor_service import AnchorCapabilities, AnchorSep6Transfer, AnchorService\n",
    "from ..anchor_service import AnchorCapabilities\nfrom ..anchor_transfer_service import (\n    AnchorDepositInstructions,\n    AnchorKycRequired,\n    AnchorNeedFields,\n    AnchorOpenUrl,\n    AnchorTransferPlan,\n    AnchorWithdrawalPayment,\n)\n",
)
replace_once(asset_details, "from ..errors import FresnicaError, WalletLockedError\n", "from ..errors import FresnicaError\n")
replace_between(
    asset_details,
    "@dataclass(frozen=True)\nclass AnchorWithdrawalRequest:\n",
    "\n\nclass Sep6TransferDialog",
    "class Sep6TransferDialog",
)
# Repair the doubled class marker produced by range replacement.
replace_once(asset_details, "class Sep6TransferDialogclass Sep6TransferDialog", "class Sep6TransferDialog")
replace_between(
    asset_details,
    "    def __init__(self, kind: str, asset_code: str, info: dict):\n",
    "    def compose(self) -> ComposeResult:\n",
'''    def __init__(self, kind: str, asset_code: str, plan: AnchorTransferPlan):\n        super().__init__()\n        self.kind = kind\n        self.asset_code = asset_code\n        self.transfer_type = plan.transfer_type\n        self.fields = [\n            (name, spec if isinstance(spec, dict) else {})\n            for name, spec in plan.user_fields.items()\n        ]\n\n'''
)
replace_between(
    asset_details,
    "    def _start_anchor(self, kind: Literal[\"deposit\", \"withdraw\"]) -> None:\n",
    "    def on_button_pressed(self, event: Button.Pressed) -> None:\n",
'''    def _start_anchor(self, kind: Literal["deposit", "withdraw"]) -> None:\n        capabilities = self._anchor_capabilities\n        service = getattr(self.runtime, "anchor_transfer_service", None) if self.runtime else None\n        if capabilities is None:\n            self.set_status("Discover anchor capabilities first (A).")\n            return\n        if service is None:\n            self.set_status("Anchor transfer workflow is unavailable in this runtime.")\n            return\n        try:\n            plan = service.plan(capabilities, kind)\n        except (FresnicaError, ValueError) as exc:\n            self.set_status(str(exc))\n            return\n        if plan.requires_fields:\n            self.app.push_screen(\n                Sep6TransferDialog(kind, self.asset.code, plan),\n                lambda values: self._begin_anchor_transfer(plan, values),\n            )\n            return\n        self._begin_anchor_transfer(plan, {})\n\n    def _begin_anchor_transfer(\n        self,\n        plan: AnchorTransferPlan,\n        fields: dict | None,\n    ) -> None:\n        if fields is None or self.runtime is None:\n            return\n        try:\n            record = self.runtime.wallet_manager.get_record()\n            state = self.runtime.wallet_manager.state(record.name)\n        except FresnicaError as exc:\n            self.set_status(str(exc))\n            return\n        if plan.requires_signing and state is WalletState.WATCH_ONLY:\n            self.set_status("Watch-only wallet cannot complete this anchor transfer.")\n            return\n        if plan.requires_signing and state is WalletState.LOCKED:\n            self.app.push_screen(\n                UnlockDialog(record.name),\n                lambda password: self._after_anchor_unlock(\n                    plan, fields, record.name, password\n                ),\n            )\n            return\n        session = (\n            self.runtime.wallet_manager.current()\n            if plan.requires_signing\n            else self.runtime.wallet_manager.view()\n        )\n        if session is None:\n            self.set_status("Wallet is locked.")\n            return\n        self.set_status(\n            f"Starting anchor {plan.kind} via {plan.protocol.upper()}..."\n        )\n        self._run_anchor_transfer(plan, fields, session.wallet)\n\n    def _after_anchor_unlock(\n        self,\n        plan: AnchorTransferPlan,\n        fields: dict,\n        wallet_name: str,\n        password: str | None,\n    ) -> None:\n        if password is None or self.runtime is None:\n            return\n        try:\n            self.runtime.wallet_manager.unlock(wallet_name, password)\n        except (FresnicaError, ValueError) as exc:\n            self.app.push_screen(\n                UnlockDialog(wallet_name, error=str(exc)),\n                lambda retry: self._after_anchor_unlock(\n                    plan, fields, wallet_name, retry\n                ),\n            )\n            return\n        self._begin_anchor_transfer(plan, fields)\n\n    @work(exclusive=True, thread=True, exit_on_error=False)\n    def _run_anchor_transfer(self, plan: AnchorTransferPlan, fields: dict, wallet) -> None:\n        try:\n            if self.runtime is None or self._anchor_capabilities is None:\n                raise ValueError("Anchor transfer context is unavailable")\n            service = getattr(self.runtime, "anchor_transfer_service", None)\n            if service is None:\n                raise ValueError("Anchor transfer workflow is unavailable")\n            record = self.runtime.wallet_manager.get_record()\n            network = get_network(record.network)\n            outcome = service.start(\n                wallet,\n                self.asset,\n                self._anchor_capabilities,\n                plan.kind,\n                network.passphrase,\n                fields=fields,\n                plan=plan,\n            )\n            self.app.call_from_thread(self._finish_anchor_transfer, outcome, None)\n        except (FresnicaError, ValueError) as exc:\n            self.app.call_from_thread(self._finish_anchor_transfer, None, exc)\n\n    def _finish_anchor_transfer(self, outcome, error) -> None:\n        if not self.is_mounted:\n            return\n        if error is not None:\n            self.set_status(f"Anchor transfer failed: {error}")\n            return\n        if isinstance(outcome, AnchorNeedFields):\n            self.app.push_screen(\n                Sep6TransferDialog(outcome.plan.kind, self.asset.code, outcome.plan),\n                lambda values: self._begin_anchor_transfer(outcome.plan, values),\n            )\n            return\n        if isinstance(outcome, AnchorOpenUrl):\n            opened = bool(webbrowser.open(outcome.url, new=2))\n            self.query_one("#asset-anchor", Static).update(\n                f"Anchor {outcome.kind} session:\\n{outcome.url}"\n            )\n            if opened:\n                self.set_status(\n                    f"Opened anchor {outcome.kind} flow in the system browser."\n                )\n            else:\n                self.set_status(\n                    "Browser did not open automatically · use the URL shown above."\n                )\n            return\n        if isinstance(outcome, AnchorKycRequired):\n            self.query_one("#asset-anchor", Static).update(\n                _anchor_payload_text(outcome.kind, outcome.payload)\n            )\n            self.set_status(\n                "Anchor requires customer information · SEP-12/KYC handoff is not exposed yet."\n            )\n            return\n        if isinstance(outcome, AnchorDepositInstructions):\n            self.query_one("#asset-anchor", Static).update(\n                _anchor_payload_text("deposit", outcome.payload)\n            )\n            self.set_status("SEP-6 deposit instructions ready.")\n            return\n        if isinstance(outcome, AnchorWithdrawalPayment):\n            self.query_one("#asset-anchor", Static).update(\n                _anchor_payload_text("withdraw", outcome.payload)\n            )\n            handler = getattr(self.app, "prepare_anchor_withdrawal", None)\n            if handler is None:\n                self.set_status("Anchor withdrawal payment pipeline is unavailable.")\n                return\n            handler(self, outcome)\n            return\n        self.set_status("Anchor returned an unsupported transfer outcome.")\n\n'''
)
replace_between(
    asset_details,
    "    @work(thread=True, exit_on_error=False)\n    def _discover_anchor(self, domain: str) -> None:\n",
    "    def _show_anchor(self, capabilities: AnchorCapabilities) -> None:\n",
'''    @work(thread=True, exit_on_error=False)\n    def _discover_anchor(self, domain: str) -> None:\n        try:\n            if self.runtime is None:\n                raise ValueError("Anchor transfer context is unavailable")\n            service = getattr(self.runtime, "anchor_transfer_service", None)\n            if service is None:\n                raise ValueError("Anchor transfer workflow is unavailable")\n            capabilities = service.discover(self.asset, domain)\n            self.app.call_from_thread(self._apply_anchor, capabilities, None)\n        except (FresnicaError, ValueError) as exc:\n            self.app.call_from_thread(self._apply_anchor, None, exc)\n\n    def _load_cached_anchor(self) -> None:\n        if self.runtime is None or not self.domain:\n            return\n        store = getattr(self.runtime, "anchor_capabilities_store", None)\n        if store is None:\n            return\n        try:\n            record = self.runtime.wallet_manager.get_record()\n            capabilities = store.get(record.network, self.asset, self.domain)\n        except FresnicaError:\n            return\n        if capabilities is not None:\n            self._show_anchor(capabilities)\n\n    def _apply_anchor(self, capabilities: AnchorCapabilities | None, error) -> None:\n        self._anchor_loading = False\n        if not self.is_mounted:\n            return\n        widget = self.query_one("#asset-anchor", Static)\n        if error is not None:\n            widget.update(f"Anchor discovery: unavailable ({error})")\n            return\n        if capabilities is None:\n            return\n        cache_error = None\n        store = getattr(self.runtime, "anchor_capabilities_store", None) if self.runtime else None\n        if store is not None:\n            try:\n                record = self.runtime.wallet_manager.get_record()\n                store.put(record.network, self.asset, capabilities)\n            except FresnicaError as exc:\n                cache_error = exc\n        self._show_anchor(capabilities)\n        if cache_error is not None:\n            self.set_status(f"Anchor discovered · cache unavailable: {cache_error}")\n        else:\n            self.set_status("Anchor capabilities updated and cached.")\n\n'''
)
replace_between(
    asset_details,
    "def _sep6_schema(info: dict) -> tuple[str | None, dict]:\n",
    "def _optional_payload_text(value) -> str | None:\n",
'''def _anchor_payload_text(kind: str, payload: dict) -> str:\n    lines = [f"SEP-6 {kind}:"]\n    how = _optional_payload_text(payload.get("how"))\n    if how:\n        lines.append(how)\n    for key, label in (\n        ("account_id", "Stellar account"),\n        ("memo_type", "Memo type"),\n        ("memo", "Memo"),\n        ("fee_fixed", "Fixed fee"),\n        ("fee_percent", "Fee percent"),\n        ("min_amount", "Minimum"),\n    ):\n        value = payload.get(key)\n        if value is not None and str(value).strip():\n            lines.append(f"{label}: {value}")\n    extra = payload.get("extra_info")\n    if isinstance(extra, dict):\n        extra = extra.get("message")\n    extra_text = _optional_payload_text(extra)\n    if extra_text:\n        lines.append(extra_text)\n    if len(lines) == 1:\n        lines.append(str(payload))\n    return "\\n".join(lines)\n\n\n'''
)

# App withdrawal handoff now consumes the product-layer outcome directly.
app = PY / "fresnica" / "tui" / "app.py"
replace_once(
    app,
    "from ..history_service import is_suspicious_claimable_activity\n",
    "from ..anchor_transfer_service import AnchorWithdrawalPayment\nfrom ..history_service import is_suspicious_claimable_activity\n",
)
replace_once(
    app,
    "    AnchorWithdrawalRequest,\n",
    "",
)
replace_once(app, "        request: AnchorWithdrawalRequest,\n", "        request: AnchorWithdrawalPayment,\n")
# There are two annotations (public + worker).
replace_once(app, "        request: AnchorWithdrawalRequest,\n", "        request: AnchorWithdrawalPayment,\n")

# ---------------------------------------------------------------------------
# Tests for the new boundaries and bounded synchronization.
# ---------------------------------------------------------------------------
anchor_transfer_test = PY / "tests" / "test_anchor_transfer_service.py"
anchor_transfer_test.write_text('''from stellar_sdk import Keypair\n\nfrom fresnica.anchor_service import (\n    AnchorCapabilities,\n    AnchorInteractiveTransfer,\n    AnchorSep6Transfer,\n)\nfrom fresnica.anchor_transfer_service import (\n    AnchorDepositInstructions,\n    AnchorKycRequired,\n    AnchorOpenUrl,\n    AnchorTransferService,\n    AnchorWithdrawalPayment,\n)\nfrom fresnica.models import Asset\nfrom fresnica.wallet import Wallet\n\n\nclass Protocol:\n    def __init__(self, sep6_payload=None):\n        self.sep6_payload = sep6_payload or {}\n        self.calls = []\n\n    def discover(self, asset, domain):\n        self.calls.append(("discover", asset, domain))\n        return AnchorCapabilities(domain=domain)\n\n    def start_sep24(self, wallet, asset, capabilities, kind, network_passphrase):\n        self.calls.append(("sep24", kind))\n        return AnchorInteractiveTransfer(kind, "https://anchor.example/session", "tx-1")\n\n    def start_sep6(self, wallet, asset, capabilities, kind, network_passphrase, fields):\n        self.calls.append(("sep6", kind, dict(fields)))\n        return AnchorSep6Transfer(kind=kind, payload=dict(self.sep6_payload), request={"asset_code": asset.code, "account": wallet.address(), **fields})\n\n\ndef _asset():\n    return Asset("XRP", Keypair.random().public_key)\n\n\ndef test_plan_prefers_usable_sep24_over_sep6():\n    protocol = Protocol()\n    service = AnchorTransferService(protocol)\n    capabilities = AnchorCapabilities(\n        domain="anchor.example",\n        sep24_url="https://anchor.example/sep24",\n        web_auth_url="https://anchor.example/auth",\n        signing_key=Keypair.random().public_key,\n        sep24_deposit=True,\n        sep6_url="https://anchor.example/sep6",\n        sep6_deposit=True,\n    )\n    wallet = Wallet.from_secret(Keypair.random().secret)\n\n    plan = service.plan(capabilities, "deposit")\n    outcome = service.start(wallet, _asset(), capabilities, "deposit", "network", plan=plan, fields={})\n\n    assert plan.protocol == "sep24"\n    assert plan.requires_signing\n    assert isinstance(outcome, AnchorOpenUrl)\n    assert protocol.calls == [("sep24", "deposit")]\n\n\ndef test_sep6_plan_owns_fields_type_and_fchain_style_withdraw_interpretation():\n    memo = "AK4SOoVW88+RFUcRN2r7D4lPgys9xn9KUAAAAAAAAAA="\n    destination = Keypair.random().public_key\n    protocol = Protocol({"account_id": destination, "memo_type": "hash", "memo": memo, "extra_info": {"message": "Send exactly once"}})\n    service = AnchorTransferService(protocol)\n    capabilities = AnchorCapabilities(\n        domain="fchain.io",\n        sep6_url="https://api.fchain.io",\n        sep6_withdraw=True,\n        sep6_withdraw_info={"enabled": True, "types": {"crypto": {"fields": {"amount": {}, "dest": {}, "dest_extra": {"optional": True}}}}},\n    )\n    wallet = Wallet.from_secret(Keypair.random().secret)\n    asset = _asset()\n\n    plan = service.plan(capabilities, "withdraw")\n    outcome = service.start(\n        wallet,\n        asset,\n        capabilities,\n        "withdraw",\n        "network",\n        plan=plan,\n        fields={"amount": "5", "dest": "rExample"},\n    )\n\n    assert plan.protocol == "sep6"\n    assert plan.transfer_type == "crypto"\n    assert plan.requires_fields\n    assert plan.requires_signing\n    assert protocol.calls[-1][2]["type"] == "crypto"\n    assert isinstance(outcome, AnchorWithdrawalPayment)\n    assert outcome.asset == asset\n    assert outcome.amount == "5"\n    assert outcome.destination == destination\n    assert outcome.memo_type == "hash"\n    assert outcome.memo == memo\n    assert outcome.extra_info == "Send exactly once"\n\n\ndef test_sep6_deposit_and_kyc_are_explicit_next_actions():\n    wallet = Wallet.from_address(Keypair.random().public_key)\n    asset = _asset()\n    capabilities = AnchorCapabilities(\n        domain="fchain.io",\n        sep6_url="https://api.fchain.io",\n        sep6_deposit=True,\n        sep6_deposit_info={"enabled": True},\n    )\n\n    deposit_service = AnchorTransferService(Protocol({"how": "Address: rDeposit, DT: 42"}))\n    deposit = deposit_service.start(wallet, asset, capabilities, "deposit", "network", fields={})\n    assert isinstance(deposit, AnchorDepositInstructions)\n    assert deposit.payload["how"].startswith("Address:")\n\n    kyc_service = AnchorTransferService(Protocol({"type": "non_interactive_customer_info_needed", "fields": ["given_name"]}))\n    kyc = kyc_service.start(wallet, asset, capabilities, "deposit", "network", fields={})\n    assert isinstance(kyc, AnchorKycRequired)\n    assert kyc.payload["fields"] == ["given_name"]\n''', encoding="utf-8")

history_sync_test = PY / "tests" / "test_history_sync.py"
history_sync_test.write_text('''from stellar_sdk import Keypair\n\nfrom fresnica.datastore import MemoryDataStore\nfrom fresnica.history_service import (\n    HistoryService,\n    SYNC_MAX_INCREMENTAL_PAGES,\n    SYNC_PAGE_LIMIT,\n)\nfrom fresnica.wallet import Wallet\n\n\ndef _record(token):\n    return {\n        "paging_token": str(token),\n        "id": str(token),\n        "transaction_hash": f"tx-{token}",\n        "type": "manage_data",\n        "created_at": "2026-08-23T00:00:00Z",\n        "name": f"entry-{token}",\n    }\n\n\ndef _page(records):\n    return {"_embedded": {"records": records}}\n\n\nclass PagingAdapter:\n    def __init__(self, pages):\n        self.pages = list(pages)\n        self.calls = []\n\n    def get_operations(self, address, limit=200, cursor=None, desc=True):\n        self.calls.append((address, limit, cursor, desc))\n        if not self.pages:\n            return _page([])\n        return _page(self.pages.pop(0))\n\n\ndef test_history_incremental_sync_pages_until_horizon_head():\n    account = Keypair.random().public_key\n    wallet = Wallet.from_address(account)\n    store = MemoryDataStore()\n    store.save_operations("mainnet", account, [_record(100)])\n    adapter = PagingAdapter([\n        [_record(token) for token in range(101, 301)],\n        [_record(token) for token in range(301, 451)],\n    ])\n    service = HistoryService(adapter, store, "mainnet")\n\n    result = service.sync_recent(wallet)\n\n    assert result.fetched_count == 350\n    assert result.caught_up is True\n    assert [call[2] for call in adapter.calls] == ["100", "300"]\n    assert all(call[3] is False for call in adapter.calls)\n    assert service.cached_operation_count(wallet) == 351\n\n\ndef test_history_incremental_sync_reports_bounded_incomplete_catch_up():\n    account = Keypair.random().public_key\n    wallet = Wallet.from_address(account)\n    store = MemoryDataStore()\n    store.save_operations("mainnet", account, [_record(1)])\n    pages = []\n    start = 2\n    for _ in range(SYNC_MAX_INCREMENTAL_PAGES):\n        pages.append([_record(token) for token in range(start, start + SYNC_PAGE_LIMIT)])\n        start += SYNC_PAGE_LIMIT\n    adapter = PagingAdapter(pages)\n    service = HistoryService(adapter, store, "mainnet")\n\n    result = service.sync_recent(wallet)\n\n    assert result.fetched_count == SYNC_MAX_INCREMENTAL_PAGES * SYNC_PAGE_LIMIT\n    assert result.caught_up is False\n    assert len(adapter.calls) == SYNC_MAX_INCREMENTAL_PAGES\n\n\ndef test_history_initial_descending_snapshot_is_at_current_head():\n    account = Keypair.random().public_key\n    wallet = Wallet.from_address(account)\n    store = MemoryDataStore()\n    adapter = PagingAdapter([[_record(token) for token in range(400, 200, -1)]])\n    service = HistoryService(adapter, store, "mainnet")\n\n    result = service.sync_recent(wallet)\n\n    assert result.fetched_count == 200\n    assert result.caught_up is True\n    assert adapter.calls[0][2] is None\n    assert adapter.calls[0][3] is True\n''', encoding="utf-8")

# Cache test now proves network separation as well as exact asset/domain scope.
cache_test = PY / "tests" / "test_anchor_cache.py"
text = cache_test.read_text(encoding="utf-8")
text = text.replace("store.put(asset, capabilities)", 'store.put("mainnet", asset, capabilities)')
text = text.replace('AnchorCapabilitiesStore(path).get(asset, "ANCHOR.EXAMPLE.")', 'AnchorCapabilitiesStore(path).get("mainnet", asset, "ANCHOR.EXAMPLE.")')
text = text.replace('store.get(Asset("USD", other_issuer), "anchor.example")', 'store.get("mainnet", Asset("USD", other_issuer), "anchor.example")')
text = text.replace('store.get(asset, "other.example")', 'store.get("mainnet", asset, "other.example")')
if 'store.get("testnet", asset, "anchor.example")' not in text:
    text += '\n    assert store.get("testnet", asset, "anchor.example") is None\n'
cache_test.write_text(text, encoding="utf-8")

# Runtime graph explicitly exposes the shared anchor transfer workflow service.
runtime_test = PY / "tests" / "test_runtime.py"
text = runtime_test.read_text(encoding="utf-8")
needle = "    assert runtime.contact_store.path == tmp_path / \"contacts.json\"\n"
if needle not in text:
    raise SystemExit("runtime test insertion point missing")
text = text.replace(needle, needle + "    assert runtime.anchor_transfer_service is not None\n", 1)
runtime_test.write_text(text, encoding="utf-8")

# Existing TUI test injects a fake protocol through the product workflow boundary.
tui_asset_test = PY / "tests" / "test_tui_asset_workflows.py"
text = tui_asset_test.read_text(encoding="utf-8")
text = text.replace(
    "from fresnica.anchor_service import AnchorCapabilities, AnchorInteractiveTransfer\n",
    "from fresnica.anchor_service import AnchorCapabilities, AnchorInteractiveTransfer\nfrom fresnica.anchor_transfer_service import AnchorTransferService\n",
)
text = text.replace(
    "            assert runtime.anchor_capabilities_store.get(balance.asset, \"anchor.example\") == capabilities\n",
    "            assert runtime.anchor_capabilities_store.get(\"testnet\", balance.asset, \"anchor.example\") == capabilities\n",
)
old_patch = '''        monkeypatch.setattr("fresnica.tui.asset_details.AnchorService", FakeAnchorService)\n        monkeypatch.setattr(\n            "fresnica.tui.asset_details.webbrowser.open",\n            lambda url, new=0: opened.append((url, new)) or True,\n        )\n'''
new_patch = '''        runtime.anchor_transfer_service = AnchorTransferService(FakeAnchorService())\n        monkeypatch.setattr(\n            "fresnica.tui.asset_details.webbrowser.open",\n            lambda url, new=0: opened.append((url, new)) or True,\n        )\n'''
if old_patch not in text:
    raise SystemExit("TUI anchor monkeypatch block not found")
text = text.replace(old_patch, new_patch, 1)
tui_asset_test.write_text(text, encoding="utf-8")

# Guard against accidental partial application.
checks = {
    PY / "fresnica" / "anchor_transfer_service.py": "class AnchorTransferService",
    PY / "fresnica" / "sync.py": "class SyncResult",
    PY / "fresnica" / "tui" / "asset_details.py": "service.plan(capabilities, kind)",
    PY / "fresnica" / "history_service.py": "SYNC_MAX_INCREMENTAL_PAGES = 5",
    PY / "tests" / "test_anchor_transfer_service.py": "test_plan_prefers_usable_sep24_over_sep6",
    PY / "tests" / "test_history_sync.py": "test_history_incremental_sync_pages_until_horizon_head",
}
for path, marker in checks.items():
    if not path.exists() or marker not in path.read_text(encoding="utf-8"):
        raise SystemExit(f"patch incomplete: {path} missing {marker!r}")

print("anchor/history architecture patch applied")

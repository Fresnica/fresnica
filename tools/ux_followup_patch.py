from pathlib import Path


BRANCH_MARKER = "from .anchor_cache import AnchorCapabilitiesStore"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8")


runtime_path = Path("reference/python/fresnica/runtime.py")
runtime = runtime_path.read_text(encoding="utf-8")
if BRANCH_MARKER in runtime:
    print("UX follow-up already applied")
    raise SystemExit(0)

runtime = replace_once(
    runtime,
    "from .asset_catalog import AssetCatalogService\nfrom .balance_service import BalanceService",
    "from .anchor_cache import AnchorCapabilitiesStore\nfrom .asset_catalog import AssetCatalogService\nfrom .balance_service import BalanceService",
    "runtime anchor cache import",
)
runtime = replace_once(
    runtime,
    '        self.asset_catalog = AssetCatalogService(self.home / "assets.json")\n        self.wallet_manager = WalletManager(self.wallet_storage)',
    '        self.asset_catalog = AssetCatalogService(self.home / "assets.json")\n        self.anchor_capabilities_store = AnchorCapabilitiesStore(self.home / "anchors.json")\n        self.wallet_manager = WalletManager(self.wallet_storage)',
    "runtime anchor cache composition",
)
write(str(runtime_path), runtime)


asset_path = Path("reference/python/fresnica/tui/asset_details.py")
asset = asset_path.read_text(encoding="utf-8")
asset = replace_once(
    asset,
    "from .trustlines import TrustlineAction, TrustlineFormDialog",
    "from .trustlines import TrustlineAction",
    "asset details trustline import",
)
asset = replace_once(
    asset,
    '        Binding("s", "send", "Send"),\n        Binding("l", "set_limit", "Set limit"),\n        Binding("x", "remove_trustline", "Remove"),',
    '        Binding("s", "send", "Send"),\n        Binding("x", "remove_trustline", "Remove"),',
    "asset details set-limit binding",
)
asset = replace_once(
    asset,
    '                if not self.asset.is_native and not self.asset.is_liquidity_pool:\n                    yield Button(Text("Set limit [L]"), id="set-limit")\n                    yield Button(Text("Remove [X]"), id="remove-trustline")',
    '                if not self.asset.is_native and not self.asset.is_liquidity_pool:\n                    yield Button(Text("Remove [X]"), id="remove-trustline")',
    "asset details set-limit button",
)
asset = replace_once(
    asset,
    '''        for selector in ("#anchor-deposit", "#anchor-withdraw"):\n            buttons = self.query(selector)\n            if buttons:\n                buttons.first().display = False\n''',
    '''        for selector in ("#anchor-deposit", "#anchor-withdraw"):\n            buttons = self.query(selector)\n            if buttons:\n                buttons.first().display = False\n        self._load_cached_anchor()\n''',
    "asset details cached anchor load",
)
asset = replace_once(
    asset,
    '''    def action_set_limit(self) -> None:\n        if self.asset.is_native or self.asset.is_liquidity_pool:\n            return\n        self.app.push_screen(\n            TrustlineFormDialog(\n                "limit",\n                asset=_asset_identity(self.balance),\n                limit=str(self.balance.raw.get("limit", "")),\n            ),\n            self._on_trustline_form,\n        )\n\n''',
    "",
    "asset details set-limit action",
)
asset = replace_once(
    asset,
    '''    def _on_trustline_form(self, action: TrustlineAction | None) -> None:\n        if action is not None and self.on_trustline_action is not None:\n            self.on_trustline_action(self, action)\n\n''',
    "",
    "asset details set-limit callback",
)
asset = replace_once(
    asset,
    '            "send": self.action_send,\n            "set-limit": self.action_set_limit,\n            "remove-trustline": self.action_remove_trustline,',
    '            "send": self.action_send,\n            "remove-trustline": self.action_remove_trustline,',
    "asset details set-limit button dispatch",
)
asset = replace_once(
    asset,
    '''    def _apply_anchor(self, capabilities: AnchorCapabilities | None, error) -> None:\n        self._anchor_loading = False\n        if not self.is_mounted:\n            return\n        widget = self.query_one("#asset-anchor", Static)\n        if error is not None:\n            widget.update(f"Anchor discovery: unavailable ({error})")\n            return\n        if capabilities is None:\n            return\n        self._anchor_capabilities = capabilities\n        parts = [f"Anchor: {capabilities.domain}"]\n        if capabilities.sep24_url:\n            methods = _methods(capabilities.sep24_deposit, capabilities.sep24_withdraw)\n            parts.append(f"Interactive transfer (SEP-24): {methods}")\n        if capabilities.sep6_url:\n            methods = _methods(capabilities.sep6_deposit, capabilities.sep6_withdraw)\n            parts.append(f"Programmatic SEP-6: {methods}")\n            if not capabilities.sep24_url:\n                parts.append("SEP-6-only KYC flow is not exposed as a partial wallet action")\n        if capabilities.web_auth_url:\n            parts.append("SEP-10 authentication: available")\n        parts.extend(f"Note: {warning}" for warning in capabilities.warnings)\n        if len(parts) == 1:\n            parts.append("No SEP-6/SEP-24 transfer service advertised")\n        widget.update("\\n".join(parts))\n\n        deposit = self.query("#anchor-deposit")\n        withdraw = self.query("#anchor-withdraw")\n        if deposit:\n            deposit.first().display = bool(\n                capabilities.sep24_deposit\n                and capabilities.web_auth_url\n                and capabilities.signing_key\n            )\n        if withdraw:\n            withdraw.first().display = bool(\n                capabilities.sep24_withdraw\n                and capabilities.web_auth_url\n                and capabilities.signing_key\n            )\n\n''',
    '''    def _load_cached_anchor(self) -> None:\n        if self.runtime is None or not self.domain:\n            return\n        store = getattr(self.runtime, "anchor_capabilities_store", None)\n        if store is None:\n            return\n        try:\n            capabilities = store.get(self.asset, self.domain)\n        except FresnicaError:\n            return\n        if capabilities is not None:\n            self._show_anchor(capabilities)\n\n    def _apply_anchor(self, capabilities: AnchorCapabilities | None, error) -> None:\n        self._anchor_loading = False\n        if not self.is_mounted:\n            return\n        widget = self.query_one("#asset-anchor", Static)\n        if error is not None:\n            widget.update(f"Anchor discovery: unavailable ({error})")\n            return\n        if capabilities is None:\n            return\n        cache_error = None\n        store = getattr(self.runtime, "anchor_capabilities_store", None) if self.runtime else None\n        if store is not None:\n            try:\n                store.put(self.asset, capabilities)\n            except FresnicaError as exc:\n                cache_error = exc\n        self._show_anchor(capabilities)\n        if cache_error is not None:\n            self.set_status(f"Anchor discovered · cache unavailable: {cache_error}")\n        else:\n            self.set_status("Anchor capabilities updated and cached.")\n\n    def _show_anchor(self, capabilities: AnchorCapabilities) -> None:\n        self._anchor_capabilities = capabilities\n        widget = self.query_one("#asset-anchor", Static)\n        parts = [f"Anchor: {capabilities.domain}"]\n        if capabilities.sep24_url:\n            methods = _methods(capabilities.sep24_deposit, capabilities.sep24_withdraw)\n            parts.append(f"Interactive transfer (SEP-24): {methods}")\n        if capabilities.sep6_url:\n            methods = _methods(capabilities.sep6_deposit, capabilities.sep6_withdraw)\n            parts.append(f"Programmatic SEP-6: {methods}")\n            if not capabilities.sep24_url:\n                parts.append("SEP-6-only KYC flow is not exposed as a partial wallet action")\n        if capabilities.web_auth_url:\n            parts.append("SEP-10 authentication: available")\n        parts.extend(f"Note: {warning}" for warning in capabilities.warnings)\n        if len(parts) == 1:\n            parts.append("No SEP-6/SEP-24 transfer service advertised")\n        widget.update("\\n".join(parts))\n\n        discover = self.query("#discover-anchor")\n        if discover:\n            discover.first().label = Text("Refresh anchor [A]")\n        deposit = self.query("#anchor-deposit")\n        withdraw = self.query("#anchor-withdraw")\n        if deposit:\n            deposit.first().display = bool(\n                capabilities.sep24_deposit\n                and capabilities.web_auth_url\n                and capabilities.signing_key\n            )\n        if withdraw:\n            withdraw.first().display = bool(\n                capabilities.sep24_withdraw\n                and capabilities.web_auth_url\n                and capabilities.signing_key\n            )\n\n''',
    "asset details anchor cache rendering",
)
asset = replace_once(
    asset,
    '            for selector in ("#set-limit", "#remove-trustline"):',
    '            for selector in ("#remove-trustline",):',
    "asset details removed trustline buttons",
)
write(str(asset_path), asset)


history_service_path = Path("reference/python/fresnica/history_service.py")
history_service = history_service_path.read_text(encoding="utf-8")
history_service = replace_once(
    history_service,
    '"""Account activity with local caching and human-readable summaries."""\n\nfrom .models import ActivityView, OperationView',
    '"""Account activity with local caching and human-readable summaries."""\n\nfrom decimal import Decimal, InvalidOperation\n\nfrom .models import ActivityView, OperationView',
    "history Decimal import",
)
history_service = replace_once(
    history_service,
    'SYNC_PAGE_LIMIT = 200\n',
    'SYNC_PAGE_LIMIT = 200\nSUSPICIOUS_NATIVE_DUST_MAX = Decimal("0.0000010")\n',
    "history suspicious dust threshold",
)
history_service = replace_once(
    history_service,
    '''def is_suspicious_claimable_activity(activity: ActivityView) -> bool:\n    """Return true only for activities made entirely of unsolicited claimables.\n\n    The previous implementation hid an entire transaction when *any* operation\n    was an unsolicited incoming claimable. That could hide legitimate sibling\n    operations such as clawbacks. A mixed transaction is therefore never\n    classified as suspicious here.\n    """\n    operations = list(activity.operations)\n    if not operations:\n        return False\n    return all(\n        item.operation_type == "create_claimable_balance"\n        and bool(item.raw.get("_fresnica_unsolicited_claimable"))\n        for item in operations\n    )\n\n''',
    '''def is_suspicious_claimable_activity(activity: ActivityView) -> bool:\n    """Recognize unsolicited claimables and their same-sender stroop bait.\n\n    A claimable-only transaction is suspicious. A mixed transaction is also\n    suspicious when every sibling operation is a tiny incoming native payment\n    from the same sender to the same claimant. Other mixed transactions remain\n    visible, so legitimate sibling operations such as clawbacks are not hidden.\n    """\n    operations = list(activity.operations)\n    claimables = [item for item in operations if _is_unsolicited_claimable(item)]\n    if not claimables:\n        return False\n    if len(claimables) == len(operations):\n        return True\n\n    sources = {\n        str(item.raw.get("source_account"))\n        for item in claimables\n        if item.raw.get("source_account")\n    }\n    claimants = {\n        str(claimant.get("destination"))\n        for item in claimables\n        for claimant in item.raw.get("claimants", []) or []\n        if isinstance(claimant, dict) and claimant.get("destination")\n    }\n    if not sources or not claimants:\n        return False\n    return all(\n        _is_unsolicited_claimable(item)\n        or _is_suspicious_native_companion(item, sources, claimants)\n        for item in operations\n    )\n\n\ndef _is_unsolicited_claimable(operation: OperationView) -> bool:\n    return (\n        operation.operation_type == "create_claimable_balance"\n        and bool(operation.raw.get("_fresnica_unsolicited_claimable"))\n    )\n\n\ndef _is_suspicious_native_companion(\n    operation: OperationView,\n    sources: set[str],\n    claimants: set[str],\n) -> bool:\n    if operation.operation_type != "payment":\n        return False\n    raw = operation.raw\n    source = raw.get("from") or raw.get("source_account")\n    if str(source or "") not in sources or str(raw.get("to") or "") not in claimants:\n        return False\n    if raw.get("asset_type") != "native":\n        return False\n    try:\n        amount = Decimal(str(raw.get("amount", "")))\n    except InvalidOperation:\n        return False\n    return Decimal("0") < amount <= SUSPICIOUS_NATIVE_DUST_MAX\n\n''',
    "history suspicious mixed transaction classifier",
)
history_service = replace_once(
    history_service,
    '        return f"{contact_names[text]} · {short_address(text)}"',
    '        return f"👤 {contact_names[text]} · {short_address(text)}"',
    "history contact marker",
)
write(str(history_service_path), history_service)


history_path = Path("reference/python/fresnica/tui/history.py")
history = history_path.read_text(encoding="utf-8")
history = replace_once(
    history,
    "from textual.containers import Horizontal, Vertical",
    "from textual.containers import Horizontal, Vertical, VerticalScroll",
    "history vertical scroll import",
)
history = replace_once(
    history,
    '''    #activity-detail { margin: 1 0; }\n    #activity-ops { height: auto; max-height: 18; }\n    #activity-actions { height: auto; margin-top: 1; align-horizontal: right; }''',
    '''    #activity-detail { margin: 1 0; }\n    #activity-ops { height: auto; min-height: 3; max-height: 18; margin-top: 1; }\n    .activity-op { width: 100%; height: auto; margin-bottom: 1; }\n    #activity-actions { height: auto; margin-top: 1; align-horizontal: right; }''',
    "history detail wrap css",
)
history = replace_once(
    history,
    '''            yield Static(text, id="activity-detail")\n            yield DataTable(id="activity-ops")\n            with Horizontal(id="activity-actions"):\n''',
    '''            yield Static(text, id="activity-detail")\n            with VerticalScroll(id="activity-ops"):\n                for index, operation in enumerate(self.operations, start=1):\n                    yield Static(self._operation_detail(index, operation), classes="activity-op")\n            with Horizontal(id="activity-actions"):\n''',
    "history detail wrap container",
)
history = replace_once(
    history,
    '''    def on_mount(self) -> None:\n        table = self.query_one("#activity-ops", DataTable)\n        if not table.columns:\n            table.add_columns("#", "Operation", "Details")\n        table.cursor_type = "row"\n        if table.row_count:\n            return\n        for index, operation in enumerate(self.operations, start=1):\n            raw = operation.raw\n            details = operation.summary\n            source = raw.get("source_account")\n            if source and source != self.account:\n                details += f" · source {_address_label(source, self.contact_names)}"\n            changes = raw.get("asset_balance_changes", []) or []\n            if changes:\n                details += f" · {len(changes)} asset change{'s' if len(changes) != 1 else ''}"\n            token = raw.get("paging_token") or raw.get("id")\n            if token:\n                details += f" · op {token}"\n            table.add_row(str(index), _operation_label(operation.operation_type), details)\n\n''',
    '''    def _operation_detail(self, index: int, operation) -> Text:\n        raw = operation.raw\n        details = operation.summary\n        source = raw.get("source_account")\n        if source and source != self.account:\n            details += f" · source {_address_label(source, self.contact_names)}"\n        changes = raw.get("asset_balance_changes", []) or []\n        if changes:\n            details += f" · {len(changes)} asset change{'s' if len(changes) != 1 else ''}"\n        token = raw.get("paging_token") or raw.get("id")\n        if token:\n            details += f" · op {token}"\n        text = Text()\n        text.append(f"#{index} {_operation_label(operation.operation_type)}", style="bold")\n        text.append("\\n")\n        text.append(details)\n        return text\n\n''',
    "history detail operation renderer",
)
history = replace_once(
    history,
    '    return f"{name} · {short_address(address)}" if name else short_address(address)',
    '    return f"👤 {name} · {short_address(address)}" if name else short_address(address)',
    "history detail contact marker",
)
write(str(history_path), history)


dex_path = Path("reference/python/fresnica/tui/dex.py")
dex = dex_path.read_text(encoding="utf-8")
dex = replace_once(
    dex,
    '''    .book-pane { width: 1fr; height: 1fr; padding: 0 1; }\n    #dex-asks, #dex-bids { height: 1fr; }''',
    '''    .book-pane { width: 1fr; height: 1fr; padding: 0 1; }\n    #bids-pane { align-horizontal: right; }\n    .bid-section { text-align: right; }\n    #dex-bids { width: auto; min-width: 34; }\n    #dex-asks, #dex-bids { height: 1fr; }''',
    "DEX bid pane alignment css",
)
dex = replace_once(
    dex,
    '        self._visible_offers: list[OpenOffer] = []\n        self._recent_trades: list[dict] = []',
    '        self._visible_offers: list[OpenOffer] = []\n        self._visible_fills = []\n        self._recent_trades: list[dict] = []',
    "DEX visible fills state",
)
dex = replace_once(
    dex,
    '''        with Horizontal(id="book-row"):\n            with Vertical(classes="book-pane"):\n                yield Label("BID · BUY", classes="dex-section")\n                yield DataTable(id="dex-bids")\n            with Vertical(classes="book-pane"):\n                yield Label("ASK · SELL", classes="dex-section")\n                yield DataTable(id="dex-asks")''',
    '''        with Horizontal(id="book-row"):\n            with Vertical(id="bids-pane", classes="book-pane"):\n                yield Label("BID · BUY", classes="dex-section bid-section")\n                yield DataTable(id="dex-bids")\n            with Vertical(id="asks-pane", classes="book-pane"):\n                yield Label("ASK · SELL", classes="dex-section")\n                yield DataTable(id="dex-asks")''',
    "DEX centered book panes",
)
dex = replace_once(
    dex,
    '        self._visible_offers = []\n        self._recent_trades = []',
    '        self._visible_offers = []\n        self._visible_fills = []\n        self._recent_trades = []',
    "DEX swap clears fills",
)
dex = replace_once(
    dex,
    '''        if store is not None:\n            store.save(settings)\n        self._update_time_columns()\n        self.refresh_market()\n\n    def action_favorite_market''',
    '''        if store is not None:\n            store.save(settings)\n        self._update_time_columns()\n        self._render_recent_trades()\n        self._render_fills()\n        zone = "local" if settings.use_local_time else "UTC"\n        suffix = self._realtime_label() if self._streams_started else "snapshot loaded"\n        self._set_market_status(f"{suffix} · {zone} time")\n\n    def action_favorite_market''',
    "DEX timezone local rerender",
)
dex = replace_once(
    dex,
    '''        fill_table = self.query_one("#dex-fills", DataTable)\n        fill_table.clear()\n        for item in fills:\n            fill_table.add_row(\n                self._time(item.last_time or item.first_time),\n                item.side.upper(),\n                format_amount(item.base_amount),\n                _stellar_ratio_text(item.price_r),\n                format_amount(item.counter_amount),\n                str(item.trade_count),\n                offer_id_label(item.user_offer_id),\n            )\n\n        self._counts = (''',
    '''        self._visible_fills = list(fills)\n        self._render_fills()\n\n        self._counts = (''',
    "DEX fills local render state",
)
dex = replace_once(
    dex,
    '''    def _render_recent_trades(self) -> None:\n        table = self.query_one("#dex-trades", DataTable)\n        table.clear()\n        for raw in self._recent_trades[:30]:\n            buy = not bool(raw.get("base_is_seller"))\n            table.add_row(\n                _stellar_decimal_text(_trade_price(raw), style="green" if buy else "red"),\n                format_amount(_decimal(raw.get("base_amount", "0"))),\n                self._time(raw.get("ledger_close_time")),\n            )\n\n    def _start_realtime''',
    '''    def _render_recent_trades(self) -> None:\n        table = self.query_one("#dex-trades", DataTable)\n        table.clear()\n        for raw in self._recent_trades[:30]:\n            buy = not bool(raw.get("base_is_seller"))\n            table.add_row(\n                _stellar_decimal_text(_trade_price(raw), style="green" if buy else "red"),\n                format_amount(_decimal(raw.get("base_amount", "0"))),\n                self._time(raw.get("ledger_close_time")),\n            )\n\n    def _render_fills(self) -> None:\n        table = self.query_one("#dex-fills", DataTable)\n        table.clear()\n        for item in self._visible_fills:\n            table.add_row(\n                self._time(item.last_time or item.first_time),\n                item.side.upper(),\n                format_amount(item.base_amount),\n                _stellar_ratio_text(item.price_r),\n                format_amount(item.counter_amount),\n                str(item.trade_count),\n                offer_id_label(item.user_offer_id),\n            )\n\n    def _start_realtime''',
    "DEX fill renderer",
)
dex = replace_once(
    dex,
    '''        for row in orderbook.get("bids", []):\n            amount = _bid_base_amount(row)\n            bids.add_row(\n                _stellar_decimal_text(amount),\n                _stellar_decimal_text(_book_price(row), style="green"),\n            )''',
    '''        for row in orderbook.get("bids", []):\n            amount = _bid_base_amount(row)\n            bids.add_row(\n                _stellar_decimal_text(amount, justify="right"),\n                _stellar_decimal_text(_book_price(row), style="green", justify="right"),\n            )''',
    "DEX bid numeric alignment",
)
dex = replace_once(
    dex,
    '''def _stellar_decimal_text(value, style: str | None = None) -> Text:\n    significant, padding = stellar_decimal_parts(value)\n    text = Text()''',
    '''def _stellar_decimal_text(\n    value,\n    style: str | None = None,\n    justify: str | None = None,\n) -> Text:\n    significant, padding = stellar_decimal_parts(value)\n    text = Text(justify=justify)''',
    "DEX decimal justify support",
)
write(str(dex_path), dex)


test_history_path = Path("reference/python/tests/test_history_ux.py")
test_history = test_history_path.read_text(encoding="utf-8")
test_history = replace_once(
    test_history,
    '''    assert not is_suspicious_claimable_activity(activity)\n\n\ndef test_contract_call_summarizes_asset_changes_and_uses_current_metadata():''',
    '''    assert not is_suspicious_claimable_activity(activity)\n\n\ndef test_claimable_plus_same_sender_two_stroop_payment_is_suspicious():\n    account = Keypair.random().public_key\n    sender = Keypair.random().public_key\n    issuer = Keypair.random().public_key\n    service = _service(\n        account,\n        [\n            {\n                "paging_token": "51",\n                "transaction_hash": "spam-with-bait",\n                "type": "create_claimable_balance",\n                "source_account": sender,\n                "amount": "16.0000000",\n                "asset": f"GMB:{issuer}",\n                "claimants": [{"destination": account, "predicate": {"unconditional": True}}],\n            },\n            {\n                "paging_token": "50",\n                "transaction_hash": "spam-with-bait",\n                "type": "payment",\n                "source_account": sender,\n                "from": sender,\n                "to": account,\n                "amount": "0.0000002",\n                "asset_type": "native",\n            },\n        ],\n    )\n\n    [activity] = service.get_activity_views(Wallet.from_address(account), refresh=False)\n\n    assert is_suspicious_claimable_activity(activity)\n    assert "Incoming claimable asset: 16 GMB" in activity.summary\n    assert "Received 0.0000002 XLM" in activity.summary\n\n\ndef test_claimable_plus_meaningful_payment_remains_visible():\n    account = Keypair.random().public_key\n    sender = Keypair.random().public_key\n    issuer = Keypair.random().public_key\n    service = _service(\n        account,\n        [\n            {\n                "paging_token": "61",\n                "transaction_hash": "mixed-payment",\n                "type": "create_claimable_balance",\n                "source_account": sender,\n                "amount": "16.0000000",\n                "asset": f"GMB:{issuer}",\n                "claimants": [{"destination": account, "predicate": {"unconditional": True}}],\n            },\n            {\n                "paging_token": "60",\n                "transaction_hash": "mixed-payment",\n                "type": "payment",\n                "source_account": sender,\n                "from": sender,\n                "to": account,\n                "amount": "1.0000000",\n                "asset_type": "native",\n            },\n        ],\n    )\n\n    [activity] = service.get_activity_views(Wallet.from_address(account), refresh=False)\n\n    assert not is_suspicious_claimable_activity(activity)\n\n\ndef test_contract_call_summarizes_asset_changes_and_uses_current_metadata():''',
    "history suspicious bait tests",
)
test_history = replace_once(
    test_history,
    '    assert "Alice ·" in summary',
    '    assert "👤 Alice ·" in summary',
    "history contact marker test",
)
write(str(test_history_path), test_history)


test_asset_path = Path("reference/python/tests/test_tui_asset_workflows.py")
test_asset = test_asset_path.read_text(encoding="utf-8")
test_asset = replace_once(
    test_asset,
    "from textual.widgets import Input, Static",
    "from textual.widgets import Static",
    "asset workflow imports",
)
test_asset = replace_once(
    test_asset,
    "from fresnica.anchor_service import AnchorCapabilities, AnchorInteractiveTransfer",
    "from fresnica.anchor_cache import AnchorCapabilitiesStore\nfrom fresnica.anchor_service import AnchorCapabilities, AnchorInteractiveTransfer",
    "asset workflow anchor cache import",
)
test_asset = replace_once(
    test_asset,
    "from fresnica.tui.trustlines import TrustlineFormDialog\n",
    "",
    "asset workflow trustline dialog import",
)
test_asset = replace_once(
    test_asset,
    '''class Runtime:\n    def __init__(self):\n        self.wallet_manager = WalletManager(MemoryWalletStorage())''',
    '''class Runtime:\n    def __init__(self, tmp_path):\n        self.anchor_capabilities_store = AnchorCapabilitiesStore(tmp_path / "anchors.json")\n        self.wallet_manager = WalletManager(MemoryWalletStorage())''',
    "asset workflow runtime cache",
)
test_asset = replace_once(
    test_asset,
    '''def test_asset_details_has_direct_trustline_actions_and_no_generic_receive():\n    async def scenario():\n        runtime = Runtime()''',
    '''def test_asset_details_hides_advanced_limit_action_but_keeps_remove(tmp_path):\n    async def scenario():\n        runtime = Runtime(tmp_path)''',
    "asset workflow set-limit test name",
)
test_asset = replace_once(
    test_asset,
    '''            assert len(screen.query("#receive")) == 0\n            assert len(screen.query("#set-limit")) == 1\n            assert len(screen.query("#remove-trustline")) == 1\n\n            await pilot.press("l")\n            await _settle(pilot)\n            assert isinstance(app.screen, TrustlineFormDialog)\n            assert str(app.screen.query_one("#asset-label", Static).render()) == (\n                f"USD:{balance.asset.issuer}"\n            )\n            app.screen.query_one("#limit", Input).value = "250"\n            await pilot.click("#review")\n            await _settle(pilot)\n            assert actions[-1][0] is screen\n            assert actions[-1][1].kind == "limit"\n            assert actions[-1][1].asset == f"USD:{balance.asset.issuer}"\n            assert actions[-1][1].limit == "250"\n\n            await pilot.press("x")''',
    '''            assert len(screen.query("#receive")) == 0\n            assert len(screen.query("#set-limit")) == 0\n            assert len(screen.query("#remove-trustline")) == 1\n\n            await pilot.press("x")''',
    "asset workflow hide set-limit assertions",
)
test_asset = replace_once(
    test_asset,
    '''def test_anchor_discovery_exposes_real_sep24_action_and_opens_browser(monkeypatch):\n    async def scenario():\n        runtime = Runtime()''',
    '''def test_anchor_discovery_is_cached_and_reused_without_network(tmp_path, monkeypatch):\n    async def scenario():\n        runtime = Runtime(tmp_path)''',
    "asset workflow anchor cache test name",
)
test_asset = replace_once(
    test_asset,
    '''        opened = []\n        started = []\n        balance = _balance()''',
    '''        opened = []\n        started = []\n        discoveries = []\n        balance = _balance()''',
    "asset workflow discovery counter",
)
test_asset = replace_once(
    test_asset,
    '''            def discover(self, asset, domain):\n                assert asset == balance.asset\n                assert domain == "anchor.example"\n                return capabilities''',
    '''            def discover(self, asset, domain):\n                assert asset == balance.asset\n                assert domain == "anchor.example"\n                discoveries.append((asset, domain))\n                return capabilities''',
    "asset workflow discovery tracking",
)
test_asset = replace_once(
    test_asset,
    '''            await pilot.press("a")\n            await _settle(pilot, 8)\n            assert screen.query_one("#anchor-deposit").display is True\n            assert screen.query_one("#anchor-withdraw").display is True\n\n            await pilot.press("d")\n            await _settle(pilot, 8)''',
    '''            await pilot.press("a")\n            await _settle(pilot, 8)\n            assert discoveries == [(balance.asset, "anchor.example")]\n            assert runtime.anchor_capabilities_store.get(balance.asset, "anchor.example") == capabilities\n            assert screen.query_one("#anchor-deposit").display is True\n            assert screen.query_one("#anchor-withdraw").display is True\n\n            await pilot.press("escape")\n            await _settle(pilot, 3)\n            cached_screen = AssetDetailsScreen(balance, runtime=runtime)\n            app.push_screen(cached_screen)\n            await _settle(pilot, 5)\n            assert discoveries == [(balance.asset, "anchor.example")]\n            assert cached_screen.query_one("#anchor-deposit").display is True\n            assert cached_screen.query_one("#anchor-withdraw").display is True\n            assert cached_screen.query_one("#discover-anchor").label.plain == "Refresh anchor [A]"\n\n            await pilot.press("d")\n            await _settle(pilot, 8)''',
    "asset workflow cached reuse",
)
test_asset = replace_once(
    test_asset,
    '''                screen.query_one("#asset-status", Static).render()\n            )''',
    '''                cached_screen.query_one("#asset-status", Static).render()\n            )''',
    "asset workflow cached screen status",
)
write(str(test_asset_path), test_asset)


test_tui_path = Path("reference/python/tests/test_tui_ux_features.py")
test_tui = test_tui_path.read_text(encoding="utf-8")
test_tui = replace_once(
    test_tui,
    '            assert app.screen.query_one("#activity-ops", DataTable).row_count == 2',
    '            assert len(app.screen.query(".activity-op")) == 2',
    "TUI wrapped activity detail test",
)
test_tui = replace_once(
    test_tui,
    '            assert "Sender ·" in str(table.get_row_at(0)[1])',
    '            assert "👤 Sender ·" in str(table.get_row_at(0)[1])',
    "TUI contact marker test",
)
write(str(test_tui_path), test_tui)


test_dex_path = Path("reference/python/tests/test_tui_dex_market_ux.py")
test_dex = test_dex_path.read_text(encoding="utf-8")
test_dex = replace_once(
    test_dex,
    '''            status = str(app.screen.query_one("#dex-status", Static).render())\n            assert "realtime order book + trades" in status\n\n            preferences = runtime.market_preferences.get(''',
    '''            status = str(app.screen.query_one("#dex-status", Static).render())\n            assert "realtime order book + trades" in status\n\n            network_calls = len(runtime.dex_service.calls)\n            await pilot.press("u")\n            await _settle(pilot, 3)\n            assert runtime.settings_store.load().use_local_time is True\n            assert len(runtime.dex_service.calls) == network_calls\n            assert [str(column.label) for column in trades.columns.values()][-1] == "Time (local)"\n            assert [str(column.label) for column in fills.columns.values()][0] == "Time (local)"\n\n            preferences = runtime.market_preferences.get(''',
    "DEX timezone no-refresh test",
)
write(str(test_dex_path), test_dex)

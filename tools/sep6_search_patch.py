from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / "reference" / "python"


def replace(path: Path, old: str, new: str, count: int = 1) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"pattern not found in {path}: {old[:120]!r}")
    path.write_text(text.replace(old, new, count), encoding="utf-8")


# --- Shared slash search -------------------------------------------------
list_search = PY / "fresnica" / "tui" / "list_search.py"
list_search.write_text('''"""Reusable slash-search overlay for long TUI lists."""

from collections.abc import Callable

from textual.app import ComposeResult
from textual.binding import Binding
from textual.containers import Vertical
from textual.screen import ModalScreen
from textual.widgets import Input, Label


class ListSearchDialog(ModalScreen[str]):
    BINDINGS = [Binding("escape", "clear", "Clear search")]

    CSS = """
    ListSearchDialog { align: center top; padding-top: 3; }
    ListSearchDialog > #search-dialog {
        width: 72;
        height: auto;
        padding: 1 2;
        border: round $accent;
        background: $surface;
    }
    #list-search-input { margin-top: 1; }
    """

    def __init__(
        self,
        initial: str = "",
        *,
        on_change: Callable[[str], None] | None = None,
        label: str = "Search list",
    ):
        super().__init__()
        self.initial = initial
        self.on_change = on_change
        self.label = label

    def compose(self) -> ComposeResult:
        with Vertical(id="search-dialog"):
            yield Label(f"{self.label} · Esc clears")
            yield Input(value=self.initial, placeholder="Type to filter...", id="list-search-input")

    def on_mount(self) -> None:
        field = self.query_one("#list-search-input", Input)
        self.set_focus(field)
        field.cursor_position = len(field.value)

    def on_input_changed(self, event: Input.Changed) -> None:
        if event.input.id == "list-search-input" and self.on_change is not None:
            self.on_change(event.value)

    def on_input_submitted(self, event: Input.Submitted) -> None:
        if event.input.id == "list-search-input":
            self.dismiss(event.value.strip())

    def action_clear(self) -> None:
        if self.on_change is not None:
            self.on_change("")
        self.dismiss("")


def matches_query(query: str, *values) -> bool:
    needle = query.strip().casefold()
    if not needle:
        return True
    return any(needle in str(value or "").casefold() for value in values)
''', encoding="utf-8")

# Asset picker: searchable curated list while keeping visible-row identity safe.
asset_picker = PY / "fresnica" / "tui" / "asset_picker.py"
replace(asset_picker,
    'from ..presentation import short_address\n',
    'from ..presentation import short_address\nfrom .list_search import ListSearchDialog, matches_query\n')
replace(asset_picker,
    '        Binding("r", "refresh", "Refresh"),\n',
    '        Binding("r", "refresh", "Refresh"),\n        Binding("/", "search", "Search"),\n')
replace(asset_picker,
    '        self._entries: list[AssetCatalogEntry] = []\n',
    '        self._entries: list[AssetCatalogEntry] = []\n        self._source_entries: list[AssetCatalogEntry] = []\n        self._search_query = ""\n')
replace(asset_picker,
    '        self._refresh_catalog()\n\n    def action_cancel',
    '        self.call_later(lambda: self.set_focus(table))\n        self._refresh_catalog()\n\n    def action_search(self) -> None:\n        self.app.push_screen(\n            ListSearchDialog(\n                self._search_query,\n                on_change=self._set_search_query,\n                label="Search assets",\n            ),\n            lambda _: self.call_later(\n                lambda: self.set_focus(self.query_one("#asset-picker-table", DataTable))\n            ),\n        )\n\n    def _set_search_query(self, query: str) -> None:\n        self._search_query = query\n        self._render_filtered_entries()\n\n    def action_cancel')
replace(asset_picker,
    '    def _render_entries(self, entries: list[AssetCatalogEntry]) -> None:\n        self._entries = list(entries)\n        table = self.query_one("#asset-picker-table", DataTable)\n        table.clear()\n        for entry in self._entries:\n',
    '    def _render_entries(self, entries: list[AssetCatalogEntry]) -> None:\n        self._source_entries = list(entries)\n        self._render_filtered_entries()\n\n    def _render_filtered_entries(self) -> None:\n        self._entries = [\n            entry\n            for entry in self._source_entries\n            if matches_query(\n                self._search_query,\n                entry.asset.display,\n                entry.asset.issuer,\n                entry.domain,\n                entry.name,\n                entry.org,\n                entry.source,\n            )\n        ]\n        table = self.query_one("#asset-picker-table", DataTable)\n        table.clear()\n        for entry in self._entries:\n')

# Market list: filter current Popular/Favorites view without changing selection mapping.
dex = PY / "fresnica" / "tui" / "dex.py"
replace(dex,
    'from .asset_picker import AssetPickerDialog\n',
    'from .asset_picker import AssetPickerDialog\nfrom .list_search import ListSearchDialog, matches_query\n')
replace(dex,
    '        Binding("space", "toggle_favorite", "Star / Unstar"),\n',
    '        Binding("space", "toggle_favorite", "Star / Unstar"),\n        Binding("/", "search", "Search"),\n')
replace(dex,
    '        self._pending_base: Asset | None = None\n',
    '        self._pending_base: Asset | None = None\n        self._search_query = ""\n')
replace(dex,
    '    def action_cancel(self) -> None:\n        self.dismiss(None)\n\n    def action_open_selected',
    '    def action_cancel(self) -> None:\n        self.dismiss(None)\n\n    def action_search(self) -> None:\n        self.app.push_screen(\n            ListSearchDialog(\n                self._search_query,\n                on_change=self._set_search_query,\n                label="Search markets",\n            ),\n            lambda _: self.call_later(\n                lambda: self.set_focus(self.query_one("#market-list", DataTable))\n            ),\n        )\n\n    def _set_search_query(self, query: str) -> None:\n        self._search_query = query\n        self._render_market_list()\n\n    def action_open_selected')
replace(dex,
    '        pairs = list(self._popular if self._tab == "popular" else favorites)\n        self._pairs = pairs\n',
    '        all_pairs = list(self._popular if self._tab == "popular" else favorites)\n        pairs = [\n            pair\n            for pair in all_pairs\n            if matches_query(\n                self._search_query,\n                _pair_label(pair),\n                _asset_identity(pair.base),\n                _asset_identity(pair.counter),\n                self._asset_source(pair.base),\n                self._asset_source(pair.counter),\n            )\n        ]\n        self._pairs = pairs\n')
replace(dex,
    '                f"{len(pairs)} markets · Enter open · Space star/unstar"\n',
    '                f"{len(pairs)}/{len(all_pairs)} markets · / search · Enter open · Space star/unstar"\n')
# Fix title contrast: tint only the background; use theme text for the label.
replace(dex,
    '    .book-section { width: 100%; padding: 0 1; }\n    .bid-section { text-align: right; color: $success; background: $success 18%; }\n    .ask-section { text-align: left; color: $error; background: $error 18%; }\n',
    '    .book-section { width: 100%; padding: 0 1; color: $text; text-style: bold; }\n    .bid-section { text-align: right; background: $success 18%; }\n    .ask-section { text-align: left; background: $error 18%; }\n')

# Contacts: filtered backing list exactly matches rendered rows.
contacts = PY / "fresnica" / "tui" / "contact_book.py"
replace(contacts,
    'from .screens import ConfirmDialog\n',
    'from .list_search import ListSearchDialog, matches_query\nfrom .screens import ConfirmDialog\n')
replace(contacts,
    '        Binding("d", "delete", "Delete"),\n',
    '        Binding("d", "delete", "Delete"),\n        Binding("/", "search", "Search"),\n')
replace(contacts,
    '        self._contacts: list[Contact] = []\n',
    '        self._contacts: list[Contact] = []\n        self._all_contacts: list[Contact] = []\n        self._search_query = ""\n')
replace(contacts,
    '    def action_close(self) -> None:\n        self.dismiss(None)\n\n    def action_add',
    '    def action_close(self) -> None:\n        self.dismiss(None)\n\n    def action_search(self) -> None:\n        self.app.push_screen(\n            ListSearchDialog(\n                self._search_query,\n                on_change=self._set_search_query,\n                label="Search contacts",\n            ),\n            lambda _: self.call_later(\n                lambda: self.set_focus(self.query_one("#contacts-table", DataTable))\n            ),\n        )\n\n    def _set_search_query(self, query: str) -> None:\n        self._search_query = query\n        self._render_contacts()\n\n    def action_add')
replace(contacts,
    '        try:\n            self._contacts = self.store.list()\n        except ContactError as exc:\n            self._contacts = []\n',
    '        try:\n            self._all_contacts = self.store.list()\n        except ContactError as exc:\n            self._all_contacts = []\n            self._contacts = []\n')
replace(contacts,
    '            return\n        for contact in self._contacts:\n            table.add_row(contact.name, short_address(contact.address), contact.memo or "")\n        self.query_one("#contacts-status", Static).update(\n            message or f"{len(self._contacts)} contacts · stored locally"\n        )\n\n    def _selected',
    '            return\n        self._render_contacts(message)\n\n    def _render_contacts(self, message: str | None = None) -> None:\n        table = self.query_one("#contacts-table", DataTable)\n        table.clear()\n        self._contacts = [\n            contact\n            for contact in self._all_contacts\n            if matches_query(\n                self._search_query, contact.name, contact.address, contact.memo\n            )\n        ]\n        for contact in self._contacts:\n            table.add_row(contact.name, short_address(contact.address), contact.memo or "")\n        suffix = f\' · filter "{self._search_query}"\' if self._search_query else ""\n        self.query_one("#contacts-status", Static).update(\n            message or f"{len(self._contacts)}/{len(self._all_contacts)} contacts · / search · stored locally{suffix}"\n        )\n\n    def _selected')

# Manage Assets: slash filtering with exact visible-row selection semantics.
trust = PY / "fresnica" / "tui" / "trustlines.py"
replace(trust,
    'from .asset_picker import AssetPickerDialog\n',
    'from .asset_picker import AssetPickerDialog\nfrom .list_search import ListSearchDialog, matches_query\n')
replace(trust,
    '        Binding("x", "remove", "Remove"),\n',
    '        Binding("x", "remove", "Remove"),\n        Binding("/", "search", "Search"),\n')
replace(trust,
    '        self._visible_lines: list[dict] = []\n',
    '        self._visible_lines: list[dict] = []\n        self._all_lines: list[dict] = []\n        self._search_query = ""\n')
replace(trust,
    '    def action_close(self) -> None:\n        self.dismiss(None)\n\n    def action_refresh',
    '    def action_close(self) -> None:\n        self.dismiss(None)\n\n    def action_search(self) -> None:\n        self.app.push_screen(\n            ListSearchDialog(\n                self._search_query,\n                on_change=self._set_search_query,\n                label="Search assets",\n            ),\n            lambda _: self.call_later(\n                lambda: self.set_focus(self.query_one("#trustlines", DataTable))\n            ),\n        )\n\n    def _set_search_query(self, query: str) -> None:\n        self._search_query = query\n        self._render_lines()\n\n    def action_refresh')
replace(trust,
    '        table.clear()\n        self._visible_lines = list(lines)\n        for raw in lines:\n',
    '        table.clear()\n        self._all_lines = list(lines)\n        self._render_lines()\n        if error is not None:\n')
# Replace the rest of _apply_trustlines after our inserted error line through status.
replace(trust,
    '        if error is not None:\n            details = getattr(error, "details", None)\n            text = f"ERROR {error}"\n            if details:\n                text += f" · DEV {details}"\n            self.set_status(text)\n            return\n        suffix = " · cached; refreshing..." if cached else ""\n        self.set_status(\n            f"{len(lines)} issued assets · A add · X remove{suffix}"\n        )\n\n    def set_status',
    '            details = getattr(error, "details", None)\n            text = f"ERROR {error}"\n            if details:\n                text += f" · DEV {details}"\n            self.set_status(text)\n            return\n        suffix = " · cached; refreshing..." if cached else ""\n        self.set_status(\n            f"{len(self._visible_lines)}/{len(self._all_lines)} issued assets · / search · A add · X remove{suffix}"\n        )\n\n    def _render_lines(self) -> None:\n        table = self.query_one("#trustlines", DataTable)\n        table.clear()\n        self._visible_lines = [\n            raw\n            for raw in self._all_lines\n            if matches_query(\n                self._search_query,\n                raw.get("asset_code"),\n                raw.get("asset_issuer"),\n                _raw_asset_identity(raw),\n            )\n        ]\n        for raw in self._visible_lines:\n            table.add_row(\n                _raw_asset_identity(raw),\n                format_amount(Decimal(str(raw.get("balance", "0")))),\n                format_amount(Decimal(str(raw.get("limit", "0")))),\n                format_amount(Decimal(str(raw.get("buying_liabilities", "0")))),\n                format_amount(Decimal(str(raw.get("selling_liabilities", "0")))),\n            )\n\n    def set_status')
# Remove the old now-stranded row rendering block between inserted render_lines call and error.
text = trust.read_text(encoding="utf-8")
stranded = '''        for raw in lines:\n            table.add_row(\n                _raw_asset_identity(raw),\n                format_amount(Decimal(str(raw.get("balance", "0")))),\n                format_amount(Decimal(str(raw.get("limit", "0")))),\n                format_amount(Decimal(str(raw.get("buying_liabilities", "0")))),\n                format_amount(Decimal(str(raw.get("selling_liabilities", "0")))),\n            )\n'''
if stranded in text:
    trust.write_text(text.replace(stranded, "", 1), encoding="utf-8")

# History: filter the cached activity view, not only the rendered cells.
history = PY / "fresnica" / "tui" / "history.py"
replace(history,
    'from .contact_book import AddContactDialog, ContactBookScreen\n',
    'from .contact_book import AddContactDialog, ContactBookScreen\nfrom .list_search import ListSearchDialog, matches_query\n')
replace(history,
    '        Binding("c", "contacts", "Contacts"),\n',
    '        Binding("c", "contacts", "Contacts"),\n        Binding("/", "search", "Search"),\n')
replace(history,
    '        self._time_column_key = None\n',
    '        self._time_column_key = None\n        self._search_query = ""\n        self._loaded_views = []\n        self._loaded_count = 0\n        self._loaded_message = ""\n        self._loaded_error = None\n')
replace(history,
    '    def action_close(self) -> None:\n        self.dismiss(None)\n\n    def action_refresh',
    '    def action_close(self) -> None:\n        self.dismiss(None)\n\n    def action_search(self) -> None:\n        self.app.push_screen(\n            ListSearchDialog(\n                self._search_query,\n                on_change=self._set_search_query,\n                label="Search activity",\n            ),\n            lambda _: self.call_later(\n                lambda: self.set_focus(self.query_one("#history-table", DataTable))\n            ),\n        )\n\n    def _set_search_query(self, query: str) -> None:\n        self._search_query = query\n        self._render_loaded()\n\n    def action_refresh')
# Turn _apply into storage + rendering helper.
replace(history,
    '    def _apply(self, views, cached_operations: int, message: str, error) -> None:\n        table = self.query_one("#history-table", DataTable)\n        table.clear()\n        settings = self._settings()\n',
    '    def _apply(self, views, cached_operations: int, message: str, error) -> None:\n        self._loaded_views = list(views)\n        self._loaded_count = cached_operations\n        self._loaded_message = message\n        self._loaded_error = error\n        self._render_loaded()\n\n    def _render_loaded(self) -> None:\n        views = self._loaded_views\n        cached_operations = self._loaded_count\n        message = self._loaded_message\n        error = self._loaded_error\n        table = self.query_one("#history-table", DataTable)\n        table.clear()\n        settings = self._settings()\n')
replace(history,
    '        self._visible_views = list(filtered[: self.limit])\n        contacts, domains = activity_metadata(self.app.runtime, self.wallet)\n        account = self.wallet.address()\n        for item in self._visible_views:\n            summary = activity_display_summary(item, account, contacts, domains)\n',
    '        contacts, domains = activity_metadata(self.app.runtime, self.wallet)\n        account = self.wallet.address()\n        if self._search_query:\n            searched = []\n            for item in filtered:\n                summary = activity_display_summary(item, account, contacts, domains)\n                if matches_query(\n                    self._search_query,\n                    summary,\n                    activity_text(item, summary, account),\n                    getattr(item, "transaction_hash", None),\n                    getattr(item, "raw", None),\n                ):\n                    searched.append(item)\n            filtered = searched\n        self._visible_views = list(filtered[: self.limit])\n        for item in self._visible_views:\n            summary = activity_display_summary(item, account, contacts, domains)\n')
replace(history,
    '            f"{message} · {len(self._visible_views)} activities shown · "\n',
    '            f"{message} · {len(self._visible_views)} activities shown · / search · "\n')

# --- SEP-6 service -------------------------------------------------------
anchor = PY / "fresnica" / "anchor_service.py"
replace(anchor,
    '    sep6_deposit: bool = False\n    sep6_withdraw: bool = False\n    sep24_deposit: bool = False\n',
    '    sep6_deposit: bool = False\n    sep6_withdraw: bool = False\n    sep6_deposit_info: dict = field(default_factory=dict)\n    sep6_withdraw_info: dict = field(default_factory=dict)\n    sep24_deposit: bool = False\n')
replace(anchor,
    '@dataclass(frozen=True)\nclass AnchorInteractiveTransfer:\n',
    '@dataclass(frozen=True)\nclass AnchorSep6Transfer:\n    kind: str\n    payload: dict\n    request: dict\n\n\n@dataclass(frozen=True)\nclass AnchorInteractiveTransfer:\n')
replace(anchor,
    '        sep6_deposit = sep6_withdraw = False\n        sep24_deposit = sep24_withdraw = False\n\n        if sep6:\n            try:\n                info = self._json(urljoin(sep6.rstrip("/") + "/", "info"))\n                sep6_deposit = _asset_enabled(info.get("deposit"), asset.code)\n                sep6_withdraw = _asset_enabled(info.get("withdraw"), asset.code)\n',
    '        sep6_deposit = sep6_withdraw = False\n        sep6_deposit_info: dict = {}\n        sep6_withdraw_info: dict = {}\n        sep24_deposit = sep24_withdraw = False\n\n        if sep6:\n            try:\n                info = self._json(urljoin(sep6.rstrip("/") + "/", "info"))\n                sep6_deposit_info = _asset_info(info.get("deposit"), asset.code) or {}\n                sep6_withdraw_info = _asset_info(info.get("withdraw"), asset.code) or {}\n                sep6_deposit = _asset_enabled(info.get("deposit"), asset.code)\n                sep6_withdraw = _asset_enabled(info.get("withdraw"), asset.code)\n')
replace(anchor,
    '            sep6_deposit=sep6_deposit,\n            sep6_withdraw=sep6_withdraw,\n            sep24_deposit=sep24_deposit,\n',
    '            sep6_deposit=sep6_deposit,\n            sep6_withdraw=sep6_withdraw,\n            sep6_deposit_info=sep6_deposit_info,\n            sep6_withdraw_info=sep6_withdraw_info,\n            sep24_deposit=sep24_deposit,\n')
replace(anchor,
    '    def start_sep24(\n',
    '''    def start_sep6(\n        self,\n        wallet,\n        asset: Asset,\n        capabilities: AnchorCapabilities,\n        kind: str,\n        network_passphrase: str,\n        fields: dict | None = None,\n    ) -> AnchorSep6Transfer:\n        if kind not in {"deposit", "withdraw"}:\n            raise ValueError(f"Unsupported anchor transfer kind: {kind}")\n        enabled = capabilities.sep6_deposit if kind == "deposit" else capabilities.sep6_withdraw\n        info = (\n            capabilities.sep6_deposit_info\n            if kind == "deposit"\n            else capabilities.sep6_withdraw_info\n        )\n        if not enabled or not capabilities.sep6_url:\n            raise AnchorError(f"SEP-6 {kind} is not available for {asset.code}")\n        if asset.is_native or asset.is_liquidity_pool or not asset.issuer:\n            raise AnchorError("SEP-6 asset must be an issued Stellar asset")\n\n        params = {"asset_code": asset.code, "account": wallet.address()}\n        for key, value in (fields or {}).items():\n            if value is not None and str(value).strip():\n                params[str(key)] = str(value).strip()\n        types = info.get("types") if isinstance(info, dict) else None\n        if kind == "withdraw" and "type" not in params and isinstance(types, dict) and len(types) == 1:\n            params["type"] = next(iter(types))\n\n        headers = None\n        if isinstance(info, dict) and bool(info.get("authentication_required", False)):\n            if not capabilities.web_auth_url or not capabilities.signing_key:\n                raise AnchorError("Anchor SEP-6 flow requires SEP-10 authentication metadata")\n            if not wallet.can_sign():\n                raise AnchorError("Watch-only wallet cannot sign SEP-10 authentication")\n            token = self._authenticate_sep10(wallet, capabilities, network_passphrase)\n            headers = {"Authorization": f"Bearer {token}"}\n\n        endpoint = urljoin(capabilities.sep6_url.rstrip("/") + "/", kind)\n        payload = self._get_json(endpoint, params=params, headers=headers, allow_403=True)\n        return AnchorSep6Transfer(kind=kind, payload=payload, request=params)\n\n    def start_sep24(\n''')
replace(anchor,
    '    def _post_json(self, url: str, *, data=None, json_body=None, headers=None) -> dict:\n',
    '''    def _get_json(self, url: str, *, params=None, headers=None, allow_403: bool = False) -> dict:\n        try:\n            kwargs = {"params": params, "timeout": self.timeout}\n            if headers:\n                kwargs["headers"] = headers\n            response = self.session.get(url, **kwargs)\n            try:\n                value = response.json()\n            except ValueError:\n                value = None\n            if allow_403 and getattr(response, "status_code", 200) == 403 and isinstance(value, dict):\n                return value\n            response.raise_for_status()\n        except requests.RequestException as exc:\n            raise NetworkError(f"Unable to call anchor endpoint {url}") from exc\n        if not isinstance(value, dict):\n            raise AnchorError(f"Anchor response from {url} is malformed")\n        return value\n\n    def _post_json(self, url: str, *, data=None, json_body=None, headers=None) -> dict:\n''')
replace(anchor,
    'def _asset_enabled(section, code: str) -> bool:\n    if not isinstance(section, dict):\n        return False\n    value = section.get(code)\n    if not isinstance(value, dict):\n        value = section.get(code.upper()) or section.get(code.lower())\n    if not isinstance(value, dict):\n        return False\n    return bool(value.get("enabled", True))\n',
    '''def _asset_info(section, code: str) -> dict | None:\n    if not isinstance(section, dict):\n        return None\n    value = section.get(code)\n    if not isinstance(value, dict):\n        value = section.get(code.upper()) or section.get(code.lower())\n    return dict(value) if isinstance(value, dict) else None\n\n\ndef _asset_enabled(section, code: str) -> bool:\n    value = _asset_info(section, code)\n    return value is not None and bool(value.get("enabled", True))\n''')

# Persist SEP-6 method metadata alongside discovered capabilities.
anchor_cache = PY / "fresnica" / "anchor_cache.py"
replace(anchor_cache,
    '                sep6_withdraw=bool(value.get("sep6_withdraw", False)),\n                sep24_deposit=bool(value.get("sep24_deposit", False)),\n',
    '                sep6_withdraw=bool(value.get("sep6_withdraw", False)),\n                sep6_deposit_info=dict(value.get("sep6_deposit_info", {})) if isinstance(value.get("sep6_deposit_info", {}), dict) else {},\n                sep6_withdraw_info=dict(value.get("sep6_withdraw_info", {})) if isinstance(value.get("sep6_withdraw_info", {}), dict) else {},\n                sep24_deposit=bool(value.get("sep24_deposit", False)),\n')
replace(anchor_cache,
    '                "sep6_withdraw": capabilities.sep6_withdraw,\n                "sep24_deposit": capabilities.sep24_deposit,\n',
    '                "sep6_withdraw": capabilities.sep6_withdraw,\n                "sep6_deposit_info": capabilities.sep6_deposit_info,\n                "sep6_withdraw_info": capabilities.sep6_withdraw_info,\n                "sep24_deposit": capabilities.sep24_deposit,\n')

# Memo type support is needed for SEP-6 anchors such as fchain XRP (hash memo).
review = PY / "fresnica" / "review.py"
replace(review,
    '    memo: str | None = None\n    contact_name: str | None = None\n',
    '    memo: str | None = None\n    memo_type: str | None = None\n    contact_name: str | None = None\n')
review_presentation = PY / "fresnica" / "review_presentation.py"
replace(review_presentation,
    '    if review.memo:\n        fields.append(ReviewField("Memo", review.memo))\n',
    '    if review.memo:\n        memo_label = f"Memo ({review.memo_type})" if review.memo_type and review.memo_type != "text" else "Memo"\n        fields.append(ReviewField(memo_label, review.memo))\n')
transfer_service = PY / "fresnica" / "transfer_service.py"
replace(transfer_service,
    '        memo: str | None = None,\n        contact_name: str | None = None,\n',
    '        memo: str | None = None,\n        memo_type: str | None = None,\n        contact_name: str | None = None,\n')
replace(transfer_service,
    '            memo=memo,\n            contact_name=contact_name,\n',
    '            memo=memo,\n            memo_type=memo_type,\n            contact_name=contact_name,\n')
builder = PY / "fresnica" / "transaction_builder_service.py"
replace(builder,
    '        memo: str | None = None,\n        contact_name: str | None = None,\n',
    '        memo: str | None = None,\n        memo_type: str | None = None,\n        contact_name: str | None = None,\n')
replace(builder,
    '            memo=memo,\n            create_destination=create_destination,\n',
    '            memo=memo,\n            memo_type=memo_type,\n            create_destination=create_destination,\n')
replace(builder,
    '            memo=memo,\n            contact_name=contact_name,\n',
    '            memo=memo,\n            memo_type=memo_type,\n            contact_name=contact_name,\n')
adapter = PY / "fresnica" / "stellar_adapter.py"
replace(adapter,
    '"""Thin boundary around Stellar Python SDK network operations."""\n\n',
    '"""Thin boundary around Stellar Python SDK network operations."""\n\nimport base64\n\n')
replace(adapter,
    '        memo: str | None = None,\n        timeout: int = 30,\n',
    '        memo: str | None = None,\n        memo_type: str | None = None,\n        timeout: int = 30,\n')
replace(adapter,
    '            if memo:\n                builder = builder.add_text_memo(memo)\n            return builder.set_timeout(timeout).build()\n        except SdkError as exc:\n',
    '''            if memo:\n                kind = (memo_type or "text").lower()\n                if kind == "text":\n                    builder = builder.add_text_memo(memo)\n                elif kind == "id":\n                    builder = builder.add_id_memo(int(memo))\n                elif kind == "hash":\n                    raw = base64.b64decode(memo, validate=True)\n                    if len(raw) != 32:\n                        raise ValueError("Hash memo must decode to exactly 32 bytes")\n                    builder = builder.add_hash_memo(raw)\n                elif kind in {"return", "return_hash"}:\n                    raw = base64.b64decode(memo, validate=True)\n                    if len(raw) != 32:\n                        raise ValueError("Return-hash memo must decode to exactly 32 bytes")\n                    builder = builder.add_return_hash_memo(raw)\n                else:\n                    raise ValueError(f"Unsupported Stellar memo type: {memo_type}")\n            return builder.set_timeout(timeout).build()\n        except (SdkError, ValueError, TypeError) as exc:\n''')

# Asset details: native SEP-6 form/instructions and handoff to the existing payment review pipeline.
asset_details = PY / "fresnica" / "tui" / "asset_details.py"
replace(asset_details,
    'from ..anchor_service import AnchorCapabilities, AnchorService\n',
    'from ..anchor_service import AnchorCapabilities, AnchorSep6Transfer, AnchorService\n')
replace(asset_details,
    '@dataclass(frozen=True)\nclass AssetDetailAction:\n    kind: AssetDetailActionKind\n    asset: str\n\n\nclass PrefilledSendDialog',
    '''@dataclass(frozen=True)\nclass AssetDetailAction:\n    kind: AssetDetailActionKind\n    asset: str\n\n\n@dataclass(frozen=True)\nclass AnchorWithdrawalRequest:\n    asset: str\n    amount: str\n    destination: str\n    memo: str | None = None\n    memo_type: str | None = None\n    anchor_domain: str | None = None\n    extra_info: str | None = None\n\n\nclass Sep6TransferDialog(ModalScreen[dict | None]):\n    BINDINGS = [("escape", "cancel", "Cancel")]\n\n    CSS = """\n    Sep6TransferDialog { align: center middle; }\n    Sep6TransferDialog > #dialog { width: 94; height: auto; max-height: 90%; padding: 1 2; border: round $accent; background: $surface; }\n    Sep6TransferDialog Input { margin-top: 1; }\n    Sep6TransferDialog .field-help { color: $text-muted; height: auto; }\n    Sep6TransferDialog #form-error { color: $error; height: auto; margin-top: 1; }\n    Sep6TransferDialog #actions { height: auto; margin-top: 1; align-horizontal: right; }\n    Sep6TransferDialog Button { margin-left: 1; }\n    """\n\n    def __init__(self, kind: str, asset_code: str, info: dict):\n        super().__init__()\n        self.kind = kind\n        self.asset_code = asset_code\n        self.info = info if isinstance(info, dict) else {}\n        self.transfer_type, fields = _sep6_schema(self.info)\n        self.fields = [\n            (name, spec if isinstance(spec, dict) else {})\n            for name, spec in fields.items()\n            if name not in {"asset_code", "account"}\n        ]\n\n    def compose(self) -> ComposeResult:\n        with Vertical(id="dialog"):\n            yield Label(f"SEP-6 {self.kind} · {self.asset_code}")\n            if self.transfer_type:\n                yield Static(f"Method: {self.transfer_type}", classes="field-help")\n            for index, (name, spec) in enumerate(self.fields):\n                optional = bool(spec.get("optional", False))\n                marker = "optional" if optional else "required"\n                description = str(spec.get("description") or "").strip()\n                yield Label(f"{name} ({marker})")\n                if description:\n                    yield Static(description, classes="field-help")\n                yield Input(placeholder=name, id=f"sep6-field-{index}")\n            yield Static("", id="form-error")\n            with Horizontal(id="actions"):\n                yield Button("Cancel [Esc]", id="cancel")\n                yield Button("Continue", id="continue", variant="primary")\n\n    def action_cancel(self) -> None:\n        self.dismiss(None)\n\n    def on_button_pressed(self, event: Button.Pressed) -> None:\n        if event.button.id == "cancel":\n            self.action_cancel()\n            return\n        if event.button.id != "continue":\n            return\n        result = {}\n        if self.transfer_type:\n            result["type"] = self.transfer_type\n        for index, (name, spec) in enumerate(self.fields):\n            value = self.query_one(f"#sep6-field-{index}", Input).value.strip()\n            if not value and not bool(spec.get("optional", False)):\n                self.query_one("#form-error", Static).update(f"{name} is required.")\n                return\n            if value:\n                result[name] = value\n        self.dismiss(result)\n\n\nclass PrefilledSendDialog''')
# Replace anchor entry methods as one block.
old_anchor_flow = '''    def _start_anchor(self, kind: Literal["deposit", "withdraw"]) -> None:\n        capabilities = self._anchor_capabilities\n        if capabilities is None:\n            self.set_status("Discover anchor capabilities first (A).")\n            return\n        enabled = capabilities.sep24_deposit if kind == "deposit" else capabilities.sep24_withdraw\n        if not enabled:\n            self.set_status(f"SEP-24 {kind} is not available for this asset.")\n            return\n        if self.runtime is None:\n            self.set_status("Anchor transfers are unavailable in this runtime.")\n            return\n        try:\n            record = self.runtime.wallet_manager.get_record()\n            state = self.runtime.wallet_manager.state(record.name)\n        except FresnicaError as exc:\n            self.set_status(str(exc))\n            return\n        if state is WalletState.WATCH_ONLY:\n            self.set_status("Watch-only wallet cannot sign SEP-10 authentication.")\n            return\n        if state is WalletState.LOCKED:\n            self.app.push_screen(\n                UnlockDialog(record.name),\n                lambda password: self._after_anchor_unlock(kind, record.name, password),\n            )\n            return\n        self._begin_anchor_transfer(kind)\n\n    def _after_anchor_unlock(self, kind: str, wallet_name: str, password: str | None) -> None:\n        if password is None or self.runtime is None:\n            return\n        try:\n            self.runtime.wallet_manager.unlock(wallet_name, password)\n        except (FresnicaError, ValueError) as exc:\n            self.app.push_screen(\n                UnlockDialog(wallet_name, error=str(exc)),\n                lambda retry: self._after_anchor_unlock(kind, wallet_name, retry),\n            )\n            return\n        self._begin_anchor_transfer(kind)\n\n    def _begin_anchor_transfer(self, kind: str) -> None:\n        self.set_status(f"Authenticating with anchor for {kind}...")\n        self._run_anchor_transfer(kind)\n'''
new_anchor_flow = '''    def _start_anchor(self, kind: Literal["deposit", "withdraw"]) -> None:\n        capabilities = self._anchor_capabilities\n        if capabilities is None:\n            self.set_status("Discover anchor capabilities first (A).")\n            return\n        sep24_enabled = (\n            capabilities.sep24_deposit if kind == "deposit" else capabilities.sep24_withdraw\n        )\n        sep24_ready = bool(\n            sep24_enabled and capabilities.web_auth_url and capabilities.signing_key\n        )\n        sep6_enabled = capabilities.sep6_deposit if kind == "deposit" else capabilities.sep6_withdraw\n        if sep24_ready:\n            self._start_sep24(kind)\n            return\n        if sep6_enabled and capabilities.sep6_url:\n            self._start_sep6(kind)\n            return\n        self.set_status(f"No usable SEP-24/SEP-6 {kind} flow is advertised for this asset.")\n\n    def _start_sep24(self, kind: str) -> None:\n        if self.runtime is None:\n            self.set_status("Anchor transfers are unavailable in this runtime.")\n            return\n        try:\n            record = self.runtime.wallet_manager.get_record()\n            state = self.runtime.wallet_manager.state(record.name)\n        except FresnicaError as exc:\n            self.set_status(str(exc))\n            return\n        if state is WalletState.WATCH_ONLY:\n            self.set_status("Watch-only wallet cannot sign SEP-10 authentication.")\n            return\n        if state is WalletState.LOCKED:\n            self.app.push_screen(\n                UnlockDialog(record.name),\n                lambda password: self._after_anchor_unlock(kind, record.name, password),\n            )\n            return\n        self._begin_anchor_transfer(kind)\n\n    def _after_anchor_unlock(self, kind: str, wallet_name: str, password: str | None) -> None:\n        if password is None or self.runtime is None:\n            return\n        try:\n            self.runtime.wallet_manager.unlock(wallet_name, password)\n        except (FresnicaError, ValueError) as exc:\n            self.app.push_screen(\n                UnlockDialog(wallet_name, error=str(exc)),\n                lambda retry: self._after_anchor_unlock(kind, wallet_name, retry),\n            )\n            return\n        self._begin_anchor_transfer(kind)\n\n    def _begin_anchor_transfer(self, kind: str) -> None:\n        self.set_status(f"Authenticating with anchor for {kind}...")\n        self._run_anchor_transfer(kind)\n\n    def _start_sep6(self, kind: str) -> None:\n        capabilities = self._anchor_capabilities\n        if capabilities is None:\n            return\n        info = capabilities.sep6_deposit_info if kind == "deposit" else capabilities.sep6_withdraw_info\n        transfer_type, fields = _sep6_schema(info)\n        visible_fields = [name for name in fields if name not in {"asset_code", "account"}]\n        if visible_fields or (kind == "withdraw" and transfer_type):\n            self.app.push_screen(\n                Sep6TransferDialog(kind, self.asset.code, info),\n                lambda values: self._begin_sep6_transfer(kind, values),\n            )\n            return\n        self._begin_sep6_transfer(kind, {})\n\n    def _begin_sep6_transfer(self, kind: str, fields: dict | None) -> None:\n        if fields is None or self.runtime is None or self._anchor_capabilities is None:\n            return\n        info = (\n            self._anchor_capabilities.sep6_deposit_info\n            if kind == "deposit"\n            else self._anchor_capabilities.sep6_withdraw_info\n        )\n        needs_signing = kind == "withdraw" or bool(info.get("authentication_required", False))\n        try:\n            record = self.runtime.wallet_manager.get_record()\n            state = self.runtime.wallet_manager.state(record.name)\n        except FresnicaError as exc:\n            self.set_status(str(exc))\n            return\n        if needs_signing and state is WalletState.WATCH_ONLY:\n            self.set_status("Watch-only wallet cannot complete this SEP-6 flow.")\n            return\n        if needs_signing and state is WalletState.LOCKED:\n            self.app.push_screen(\n                UnlockDialog(record.name),\n                lambda password: self._after_sep6_unlock(kind, fields, record.name, password),\n            )\n            return\n        session = self.runtime.wallet_manager.current() if needs_signing else self.runtime.wallet_manager.view()\n        if session is None:\n            self.set_status("Wallet is locked.")\n            return\n        self.set_status(f"Requesting SEP-6 {kind} instructions...")\n        self._run_sep6_transfer(kind, fields, session.wallet)\n\n    def _after_sep6_unlock(self, kind: str, fields: dict, wallet_name: str, password: str | None) -> None:\n        if password is None or self.runtime is None:\n            return\n        try:\n            self.runtime.wallet_manager.unlock(wallet_name, password)\n        except (FresnicaError, ValueError) as exc:\n            self.app.push_screen(\n                UnlockDialog(wallet_name, error=str(exc)),\n                lambda retry: self._after_sep6_unlock(kind, fields, wallet_name, retry),\n            )\n            return\n        self._begin_sep6_transfer(kind, fields)\n'''
replace(asset_details, old_anchor_flow, new_anchor_flow)
# Add worker before existing SEP24 worker.
replace(asset_details,
    '    @work(exclusive=True, thread=True, exit_on_error=False)\n    def _run_anchor_transfer(self, kind: str) -> None:\n',
    '''    @work(exclusive=True, thread=True, exit_on_error=False)\n    def _run_sep6_transfer(self, kind: str, fields: dict, wallet) -> None:\n        try:\n            if self.runtime is None or self._anchor_capabilities is None:\n                raise ValueError("Anchor transfer context is unavailable")\n            network = get_network(self.runtime.wallet_manager.get_record().network)\n            transfer = AnchorService().start_sep6(\n                wallet,\n                self.asset,\n                self._anchor_capabilities,\n                kind,\n                network.passphrase,\n                fields,\n            )\n            self.app.call_from_thread(self._finish_sep6_transfer, transfer, None)\n        except (FresnicaError, ValueError) as exc:\n            self.app.call_from_thread(self._finish_sep6_transfer, None, exc)\n\n    def _finish_sep6_transfer(self, transfer: AnchorSep6Transfer | None, error) -> None:\n        if not self.is_mounted:\n            return\n        if error is not None:\n            self.set_status(f"SEP-6 transfer failed: {error}")\n            return\n        assert transfer is not None\n        self.query_one("#asset-anchor", Static).update(_sep6_transfer_text(transfer))\n        response_type = str(transfer.payload.get("type") or "")\n        if response_type in {\n            "non_interactive_customer_info_needed",\n            "customer_info_status",\n        }:\n            self.set_status("Anchor requires customer information · SEP-12/KYC handoff is not exposed yet.")\n            return\n        if transfer.kind == "deposit":\n            self.set_status("SEP-6 deposit instructions ready.")\n            return\n        destination = str(transfer.payload.get("account_id") or "").strip()\n        amount = str(transfer.request.get("amount") or "").strip()\n        if not destination or not amount:\n            self.set_status("SEP-6 withdraw response is missing account_id or requested amount.")\n            return\n        handler = getattr(self.app, "prepare_anchor_withdrawal", None)\n        if handler is None:\n            self.set_status("Anchor withdrawal payment pipeline is unavailable.")\n            return\n        extra = transfer.payload.get("extra_info")\n        if isinstance(extra, dict):\n            extra = extra.get("message")\n        handler(\n            self,\n            AnchorWithdrawalRequest(\n                asset=_asset_identity(self.balance),\n                amount=amount,\n                destination=destination,\n                memo=_optional_payload_text(transfer.payload.get("memo")),\n                memo_type=_optional_payload_text(transfer.payload.get("memo_type")),\n                anchor_domain=self._anchor_capabilities.domain,\n                extra_info=_optional_payload_text(extra),\n            ),\n        )\n\n    @work(exclusive=True, thread=True, exit_on_error=False)\n    def _run_anchor_transfer(self, kind: str) -> None:\n''')
replace(asset_details,
    '            if not capabilities.sep24_url:\n                parts.append("SEP-6-only KYC flow is not exposed as a partial wallet action")\n',
    '')
replace(asset_details,
    '                capabilities.sep24_deposit\n                and capabilities.web_auth_url\n                and capabilities.signing_key\n            )\n',
    '                (capabilities.sep24_deposit and capabilities.web_auth_url and capabilities.signing_key)\n                or capabilities.sep6_deposit\n            )\n', 1)
replace(asset_details,
    '                capabilities.sep24_withdraw\n                and capabilities.web_auth_url\n                and capabilities.signing_key\n            )\n',
    '                (capabilities.sep24_withdraw and capabilities.web_auth_url and capabilities.signing_key)\n                or capabilities.sep6_withdraw\n            )\n', 1)
# Append SEP6 formatting helpers before _asset_identity.
replace(asset_details,
    '\ndef _asset_identity(balance: BalanceView) -> str:\n',
    '''\ndef _sep6_schema(info: dict) -> tuple[str | None, dict]:\n    if not isinstance(info, dict):\n        return None, {}\n    types = info.get("types")\n    if isinstance(types, dict) and types:\n        transfer_type = next(iter(types))\n        spec = types.get(transfer_type)\n        fields = spec.get("fields", {}) if isinstance(spec, dict) else {}\n        return transfer_type, fields if isinstance(fields, dict) else {}\n    fields = info.get("fields", {})\n    return None, fields if isinstance(fields, dict) else {}\n\n\ndef _sep6_transfer_text(transfer: AnchorSep6Transfer) -> str:\n    payload = transfer.payload\n    lines = [f"SEP-6 {transfer.kind}:"]\n    how = _optional_payload_text(payload.get("how"))\n    if how:\n        lines.append(how)\n    for key, label in (\n        ("account_id", "Stellar account"),\n        ("memo_type", "Memo type"),\n        ("memo", "Memo"),\n        ("fee_fixed", "Fixed fee"),\n        ("fee_percent", "Fee percent"),\n        ("min_amount", "Minimum"),\n    ):\n        value = payload.get(key)\n        if value is not None and str(value).strip():\n            lines.append(f"{label}: {value}")\n    extra = payload.get("extra_info")\n    if isinstance(extra, dict):\n        extra = extra.get("message")\n    extra_text = _optional_payload_text(extra)\n    if extra_text:\n        lines.append(extra_text)\n    if len(lines) == 1:\n        lines.append(str(payload))\n    return "\\n".join(lines)\n\n\ndef _optional_payload_text(value) -> str | None:\n    return str(value).strip() if value is not None and str(value).strip() else None\n\n\ndef _asset_identity(balance: BalanceView) -> str:\n''')

# App handoff into the existing reviewed/signed/submitted transfer pipeline.
app = PY / "fresnica" / "tui" / "app.py"
replace(app,
    'from .asset_details import AssetDetailAction, AssetDetailsScreen, PrefilledSendDialog\n',
    'from .asset_details import (\n    AnchorWithdrawalRequest,\n    AssetDetailAction,\n    AssetDetailsScreen,\n    PrefilledSendDialog,\n)\n')
insert_point = '''    def _visible_balance_views(self):\n'''
anchor_withdraw_code = '''    def prepare_anchor_withdrawal(\n        self,\n        screen: AssetDetailsScreen,\n        request: AnchorWithdrawalRequest,\n    ) -> None:\n        if not screen.is_mounted:\n            return\n        try:\n            record = self.runtime.wallet_manager.get_record()\n        except WalletNotFoundError:\n            screen.set_status("No wallet selected.")\n            return\n        if not self._ensure_write_clear(record, screen=screen):\n            return\n        screen.set_status("Preparing anchor withdrawal payment...")\n        self._prepare_anchor_withdrawal(screen, request)\n\n    @work(thread=True, exit_on_error=False)\n    def _prepare_anchor_withdrawal(\n        self,\n        screen: AssetDetailsScreen,\n        request: AnchorWithdrawalRequest,\n    ) -> None:\n        try:\n            manager = self.runtime.wallet_manager\n            record = manager.get_record()\n            session = manager.current()\n            if session is None or session.record.name != record.name:\n                raise WalletLockedError(f'Wallet "{record.name}" is locked')\n            services = self.runtime.services_for(record.network)\n            prepared = services.transfer_service.prepare(\n                wallet_name=record.name,\n                wallet=session.wallet,\n                destination=request.destination,\n                asset=request.asset,\n                amount=request.amount,\n                memo=request.memo,\n                memo_type=request.memo_type,\n            )\n            self.call_from_thread(\n                self._show_anchor_withdrawal_review,\n                screen,\n                session.wallet,\n                services,\n                prepared,\n                record.network,\n                request,\n            )\n        except (FresnicaError, ValueError) as exc:\n            self.call_from_thread(screen.set_status, f"Unable to prepare anchor withdrawal: {exc}")\n\n    def _show_anchor_withdrawal_review(\n        self,\n        screen,\n        wallet,\n        services,\n        prepared,\n        network: str,\n        request: AnchorWithdrawalRequest,\n    ) -> None:\n        if screen.is_mounted:\n            suffix = f" · {request.extra_info}" if request.extra_info else ""\n            screen.set_status(f"Anchor withdrawal ready for Stellar transaction review{suffix}")\n        self._show_review(wallet, services, prepared, network)\n\n'''
replace(app, insert_point, anchor_withdraw_code + insert_point)

# --- Tests ---------------------------------------------------------------
anchor_test = PY / "tests" / "test_anchor_service.py"
replace(anchor_test,
    'from fresnica.anchor_service import AnchorCapabilities, AnchorError, AnchorService\n',
    'from fresnica.anchor_service import AnchorCapabilities, AnchorError, AnchorService\n')
# Append focused SEP-6 compatibility tests using fchain-style payloads.
with anchor_test.open("a", encoding="utf-8") as f:
    f.write(r'''

class Sep6Session:
    def __init__(self, payload):
        self.payload = payload
        self.calls = []

    def get(self, url, params=None, headers=None, timeout=None):
        self.calls.append((url, params, headers, timeout))
        return Response(json_body=self.payload)


def test_sep6_accepts_fchain_style_deposit_instructions_without_structured_address():
    issuer = Keypair.random().public_key
    wallet = Wallet.from_address(Keypair.random().public_key)
    session = Sep6Session(
        {
            "how": "Address: rDepositAddress, DT: 100005077",
            "extra_info": {"message": "You MUST include the DestinationTag."},
        }
    )
    capabilities = AnchorCapabilities(
        domain="fchain.io",
        sep6_url="https://api.fchain.io",
        sep6_deposit=True,
        sep6_deposit_info={"enabled": True, "fee_fixed": "0", "fee_percent": "0"},
    )

    transfer = AnchorService(session=session).start_sep6(
        wallet,
        Asset("XRP", issuer),
        capabilities,
        "deposit",
        get_network("mainnet").passphrase,
    )

    assert transfer.payload["how"].startswith("Address:")
    assert transfer.request == {"asset_code": "XRP", "account": wallet.address()}
    assert session.calls[0][0] == "https://api.fchain.io/deposit"


def test_sep6_withdraw_preserves_anchor_hash_memo_and_request_fields():
    issuer = Keypair.random().public_key
    wallet = Wallet.from_secret(Keypair.random().secret)
    memo = "AK4SOoVW88+RFUcRN2r7D4lPgys9xn9KUAAAAAAAAAA="
    session = Sep6Session(
        {
            "account_id": Keypair.random().public_key,
            "memo_type": "hash",
            "memo": memo,
            "fee_fixed": 0.001,
            "fee_percent": 0.1,
        }
    )
    capabilities = AnchorCapabilities(
        domain="fchain.io",
        sep6_url="https://api.fchain.io",
        sep6_withdraw=True,
        sep6_withdraw_info={
            "enabled": True,
            "types": {"crypto": {"fields": {"amount": {}, "dest": {}, "dest_extra": {"optional": True}}}},
        },
    )

    transfer = AnchorService(session=session).start_sep6(
        wallet,
        Asset("XRP", issuer),
        capabilities,
        "withdraw",
        get_network("mainnet").passphrase,
        {"amount": "5", "dest": "rExample", "dest_extra": "123"},
    )

    assert transfer.request["type"] == "crypto"
    assert transfer.request["amount"] == "5"
    assert transfer.payload["memo_type"] == "hash"
    assert transfer.payload["memo"] == memo
''')

memo_test = PY / "tests" / "test_anchor_memo_types.py"
memo_test.write_text(r'''import base64
from decimal import Decimal

from stellar_sdk import Account, Keypair

from fresnica.models import Asset
from fresnica.network import get_network
from fresnica.stellar_adapter import StellarAdapter


class Server:
    def __init__(self, account):
        self.account = account

    def load_account(self, source):
        assert source == self.account.account_id
        return self.account


def test_anchor_hash_memo_builds_real_hash_memo():
    keypair = Keypair.random()
    adapter = StellarAdapter(get_network("testnet"))
    adapter.server = Server(Account(keypair.public_key, 1))
    memo_bytes = bytes(range(32))
    memo = base64.b64encode(memo_bytes).decode("ascii")

    envelope = adapter.build_payment(
        source=keypair.public_key,
        destination=Keypair.random().public_key,
        asset=Asset("XLM"),
        amount="1",
        base_fee=100,
        memo=memo,
        memo_type="hash",
    )

    assert envelope.transaction.memo.memo_hash == memo_bytes
''', encoding="utf-8")

search_test = PY / "tests" / "test_tui_list_search.py"
search_test.write_text(r'''import asyncio

from stellar_sdk import Keypair
from textual.app import App
from textual.widgets import DataTable

from fresnica.contacts import ContactStore
from fresnica.tui.contact_book import ContactBookScreen
from fresnica.tui.list_search import ListSearchDialog, matches_query


def test_matches_query_is_case_insensitive_across_fields():
    assert matches_query("xrp", "XRP", "fchain.io")
    assert matches_query("fchain", "XRP", "fchain.io")
    assert not matches_query("aqua", "XRP", "fchain.io")


def test_contact_slash_search_filters_visible_selection_safely(tmp_path):
    async def scenario():
        store = ContactStore(tmp_path / "contacts.json")
        store.add("Alice", Keypair.random().public_key)
        store.add("Bob", Keypair.random().public_key)
        app = App()
        async with app.run_test(size=(100, 30)) as pilot:
            screen = ContactBookScreen(store)
            app.push_screen(screen)
            await pilot.pause(0.1)
            table = screen.query_one("#contacts-table", DataTable)
            assert table.row_count == 2

            await pilot.press("/")
            await pilot.pause(0.05)
            assert isinstance(app.screen, ListSearchDialog)
            await pilot.press("b", "o", "b")
            await pilot.pause(0.05)
            assert table.row_count == 1
            assert screen._contacts[0].name == "Bob"

            await pilot.press("enter")
            await pilot.pause(0.05)
            assert app.screen is screen
            assert table.row_count == 1

            await pilot.press("/")
            await pilot.pause(0.03)
            await pilot.press("escape")
            await pilot.pause(0.05)
            assert table.row_count == 2
            assert [item.name for item in screen._contacts] == ["Alice", "Bob"]

    asyncio.run(scenario())
''', encoding="utf-8")

# Update cache test to prove SEP-6 request metadata survives reuse.
cache_test = PY / "tests" / "test_anchor_cache.py"
replace(cache_test,
    '        sep6_withdraw=True,\n',
    '        sep6_withdraw=True,\n        sep6_deposit_info={"enabled": True},\n        sep6_withdraw_info={"enabled": True, "types": {"crypto": {"fields": {"dest": {}}}}},\n')

print("SEP-6/search patch applied")

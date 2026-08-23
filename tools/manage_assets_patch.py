from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / "reference" / "python"


def replace(path, old, new):
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"pattern not found in {path}: {old[:80]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


# DEX: focus the only actionable row selector and give the two book-side titles
# a visible, theme-aware strip without bringing back focusable DataTables.
dex = PY / "fresnica" / "tui" / "dex.py"
replace(
    dex,
    '    .dex-section { height: 1; text-style: bold; }\n    #book-row { height: 2fr; min-height: 8; }\n    .book-pane { width: 1fr; height: 1fr; padding: 0 1; }\n    .bid-section { width: 100%; text-align: right; }\n',
    '    .dex-section { height: 1; text-style: bold; }\n    #book-row { height: 2fr; min-height: 8; }\n    .book-pane { width: 1fr; height: 1fr; padding: 0 1; }\n    .book-section { width: 100%; padding: 0 1; }\n    .bid-section { text-align: right; color: $success; background: $success 18%; }\n    .ask-section { text-align: left; color: $error; background: $error 18%; }\n',
)
replace(
    dex,
    '                yield Label("BID · BUY", classes="dex-section bid-section")\n                yield Static("", id="dex-bids")\n            with Vertical(id="asks-pane", classes="book-pane"):\n                yield Label("ASK · SELL", classes="dex-section")\n',
    '                yield Label("BID · BUY", classes="dex-section book-section bid-section")\n                yield Static("", id="dex-bids")\n            with Vertical(id="asks-pane", classes="book-pane"):\n                yield Label("ASK · SELL", classes="dex-section book-section ask-section")\n',
)
replace(
    dex,
    '        fills.add_columns(self._time_column(), "Side", "Amount", "Price", "Total", "Fills", "Offer")\n        fills.cursor_type = "row"\n        self.call_later(self._update_pair_labels)\n        self.refresh_market()\n',
    '        fills.add_columns(self._time_column(), "Side", "Amount", "Price", "Total", "Fills", "Offer")\n        fills.cursor_type = "row"\n        self.call_later(self._update_pair_labels)\n        self.call_later(lambda: self.set_focus(offers))\n        self.refresh_market()\n',
)

# User-facing information architecture: keep Trustline/ChangeTrust in the domain
# layer, but present the screen as asset management. Set-limit remains available
# internally, not as a normal TUI action.
trust = PY / "fresnica" / "tui" / "trustlines.py"
replace(trust, '        Binding("e", "edit", "Set limit"),\n', '')
replace(trust, '        yield Static("Stellar trustlines", id="trust-title")\n        yield Static("Loading trustlines...", id="trust-status")\n', '        yield Static("Manage Assets", id="trust-title")\n        yield Static("Loading issued assets...", id="trust-status")\n')
replace(trust, '                title="Choose asset for trustline",\n', '                title="Choose asset to add",\n')
replace(trust, '        self.set_status("Refreshing trustlines...")\n', '        self.set_status("Refreshing issued assets...")\n')
replace(
    trust,
    '            f"{len(lines)} trustlines · A add · E set limit · X remove{suffix}"\n',
    '            f"{len(lines)} issued assets · A add · X remove{suffix}"\n',
)

app = PY / "fresnica" / "tui" / "app.py"
replace(app, '        Binding("t", "trustlines", "Trustlines"),\n', '        Binding("t", "trustlines", "Manage Assets"),\n')
replace(app, '            self._show_notice("No wallet", "Add or import a wallet before managing trustlines.")\n', '            self._show_notice("No wallet", "Add or import a wallet before managing assets.")\n')
replace(app, '            self._show_notice("No wallet", "Add or import a wallet before changing trustlines.")\n', '            self._show_notice("No wallet", "Add or import a wallet before changing assets.")\n')
replace(app, '            screen.set_status("Watch-only wallet · trustlines are read-only.")\n', '            screen.set_status("Watch-only wallet · issued assets are read-only.")\n')
replace(app, '                "This wallet can inspect trustlines but cannot sign ChangeTrust operations.",\n', '                "This wallet can inspect issued assets but cannot sign ChangeTrust operations.",\n')

# Keep StellarExpert ranked recommendations for DEX market discovery. Add a
# separate SEP-42 curated cache for the asset picker, mirroring Fex sources:
# Lobstr, Soroswap and StellarExpert.
catalog = PY / "fresnica" / "asset_catalog.py"
replace(
    catalog,
    'STELLAR_EXPERT_ASSETS = "https://api.stellar.expert/explorer/public/asset"\nREQUEST_TIMEOUT = 5\nMAX_RECOMMENDED = 50\n',
    'STELLAR_EXPERT_ASSETS = "https://api.stellar.expert/explorer/public/asset"\nASSET_LIST_CATALOGUE = "https://stellar-asset-lists.github.io/index/"\nREQUEST_TIMEOUT = 5\nMAX_RECOMMENDED = 50\nMAX_CURATED_PER_SOURCE = 50\n_CURATED_SOURCE_ORDER = ("lobstr", "soroswap", "stellar-expert")\n',
)
replace(
    catalog,
    '    def recommended(\n        self,\n        network: str,\n        limit: int = 30,\n        refresh: bool = True,\n    ) -> list[AssetCatalogEntry]:\n',
    '''    @property\n    def curated_path(self) -> Path:\n        suffix = self.path.suffix or ".json"\n        return self.path.with_name(f"{self.path.stem}.curated{suffix}")\n\n    def cached_curated(self, network: str) -> list[AssetCatalogEntry]:\n        return self._read_cache(self.curated_path, network)\n\n    def curated(\n        self,\n        network: str,\n        limit: int = MAX_CURATED_PER_SOURCE,\n        refresh: bool = True,\n    ) -> list[AssetCatalogEntry]:\n        limit = max(1, min(int(limit), MAX_CURATED_PER_SOURCE))\n        cached = self.cached_curated(network)\n        if network != "mainnet" or not refresh:\n            return cached\n        try:\n            fresh, failures = self._fetch_curated(limit)\n        except (requests.RequestException, ValueError, NetworkError):\n            return cached\n        if failures and len(cached) > 1:\n            return cached\n        if fresh:\n            self._save_to(self.curated_path, fresh)\n            return [AssetCatalogEntry(Asset("XLM"), source="native"), *fresh]\n        return cached\n\n    def recommended(\n        self,\n        network: str,\n        limit: int = 30,\n        refresh: bool = True,\n    ) -> list[AssetCatalogEntry]:\n''',
)
replace(
    catalog,
    '    def _fetch_ranked(self, limit: int) -> list[AssetCatalogEntry]:\n',
    '''    def _read_cache(self, path: Path, network: str) -> list[AssetCatalogEntry]:\n        if network != "mainnet":\n            return [AssetCatalogEntry(Asset("XLM"), source="native")]\n        try:\n            raw = json.loads(path.read_text(encoding="utf-8"))\n        except FileNotFoundError:\n            return [AssetCatalogEntry(Asset("XLM"), source="native")]\n        except (OSError, ValueError, TypeError) as exc:\n            raise AssetCatalogError(f"Unable to read asset catalog: {path}") from exc\n        if not isinstance(raw, dict) or raw.get("version") != 1:\n            raise AssetCatalogError("Asset catalog cache is malformed")\n        entries = [AssetCatalogEntry(Asset("XLM"), source="native")]\n        seen = {entries[0].asset}\n        for item in raw.get("assets", []):\n            parsed = _entry_from_json(item)\n            if parsed is not None and parsed.asset not in seen:\n                entries.append(parsed)\n                seen.add(parsed.asset)\n        return entries\n\n    def _fetch_curated(self, limit: int) -> tuple[list[AssetCatalogEntry], int]:\n        response = self.session.get(ASSET_LIST_CATALOGUE, params=None, timeout=self.timeout)\n        response.raise_for_status()\n        descriptors = response.json()\n        if not isinstance(descriptors, list):\n            raise NetworkError("Asset-list catalogue response is malformed")\n\n        selected: dict[str, dict] = {}\n        for descriptor in descriptors:\n            if not isinstance(descriptor, dict):\n                continue\n            source = _curated_source(descriptor)\n            url = descriptor.get("url")\n            if source and isinstance(url, str) and url:\n                selected[source] = descriptor\n\n        entries: list[AssetCatalogEntry] = []\n        positions: dict[Asset, int] = {}\n        failures = 0\n        for source in _CURATED_SOURCE_ORDER:\n            descriptor = selected.get(source)\n            if descriptor is None:\n                failures += 1\n                continue\n            try:\n                list_response = self.session.get(\n                    str(descriptor["url"]), params=None, timeout=self.timeout\n                )\n                list_response.raise_for_status()\n                payload = list_response.json()\n                assets = payload.get("assets", []) if isinstance(payload, dict) else []\n                if not isinstance(assets, list):\n                    raise NetworkError("Curated asset-list response is malformed")\n            except (requests.RequestException, ValueError, NetworkError):\n                failures += 1\n                continue\n            for raw in assets[:limit]:\n                entry = _entry_from_sep42(raw, source)\n                if entry is None:\n                    continue\n                position = positions.get(entry.asset)\n                if position is None:\n                    positions[entry.asset] = len(entries)\n                    entries.append(entry)\n                else:\n                    entries[position] = _merge_entries(entries[position], entry)\n        return entries, failures\n\n    def _fetch_ranked(self, limit: int) -> list[AssetCatalogEntry]:\n''',
)
replace(
    catalog,
    '    def _save(self, entries: list[AssetCatalogEntry]) -> None:\n        temporary = self.path.with_suffix(self.path.suffix + ".tmp")\n',
    '    def _save(self, entries: list[AssetCatalogEntry]) -> None:\n        self._save_to(self.path, entries)\n\n    def _save_to(self, path: Path, entries: list[AssetCatalogEntry]) -> None:\n        temporary = path.with_suffix(path.suffix + ".tmp")\n',
)
replace(catalog, '            self.path.parent.mkdir(parents=True, exist_ok=True)\n', '            path.parent.mkdir(parents=True, exist_ok=True)\n')
replace(catalog, '            os.replace(temporary, self.path)\n', '            os.replace(temporary, path)\n')
replace(catalog, '            raise AssetCatalogError(f"Unable to write asset catalog: {self.path}") from exc\n', '            raise AssetCatalogError(f"Unable to write asset catalog: {path}") from exc\n')
# Reuse the generalized cache reader for the original ranked cache too.
old_cached = '''    def cached(self, network: str) -> list[AssetCatalogEntry]:\n        if network != "mainnet":\n            return [AssetCatalogEntry(Asset("XLM"), source="native")]\n        try:\n            raw = json.loads(self.path.read_text(encoding="utf-8"))\n        except FileNotFoundError:\n            return [AssetCatalogEntry(Asset("XLM"), source="native")]\n        except (OSError, ValueError, TypeError) as exc:\n            raise AssetCatalogError(f"Unable to read asset catalog: {self.path}") from exc\n        if not isinstance(raw, dict) or raw.get("version") != 1:\n            raise AssetCatalogError("Asset catalog cache is malformed")\n        entries = [AssetCatalogEntry(Asset("XLM"), source="native")]\n        for item in raw.get("assets", []):\n            parsed = _entry_from_json(item)\n            if parsed is not None and parsed.asset not in {entry.asset for entry in entries}:\n                entries.append(parsed)\n        return entries\n'''
replace(catalog, old_cached, '    def cached(self, network: str) -> list[AssetCatalogEntry]:\n        return self._read_cache(self.path, network)\n')
# Append SEP-42 parsing/merge helpers before _entry_json.
replace(
    catalog,
    '\ndef _entry_json(entry: AssetCatalogEntry) -> dict:\n',
    '''\ndef _curated_source(descriptor: dict) -> str | None:\n    text = f"{descriptor.get('name', '')} {descriptor.get('provider', '')}".lower()\n    if "lobstr" in text:\n        return "lobstr"\n    if "soroswap" in text:\n        return "soroswap"\n    if "stellarexpert" in text or "stellar expert" in text:\n        return "stellar-expert"\n    return None\n\n\ndef _entry_from_sep42(raw, source: str) -> AssetCatalogEntry | None:\n    if not isinstance(raw, dict):\n        return None\n    code = raw.get("code")\n    issuer = raw.get("issuer")\n    if not isinstance(code, str) or not isinstance(issuer, str):\n        return None\n    try:\n        asset = Asset(code, issuer)\n    except (FresnicaError, ValueError):\n        return None\n    return AssetCatalogEntry(\n        asset=asset,\n        domain=_optional_text(raw.get("domain")),\n        name=_optional_text(raw.get("name")),\n        org=_optional_text(raw.get("org")),\n        source=source,\n    )\n\n\ndef _merge_entries(left: AssetCatalogEntry, right: AssetCatalogEntry) -> AssetCatalogEntry:\n    sources = []\n    for source in (*left.source.split("+"), *right.source.split("+")):\n        if source and source not in sources:\n            sources.append(source)\n    return AssetCatalogEntry(\n        asset=left.asset,\n        domain=left.domain or right.domain,\n        name=left.name or right.name,\n        org=left.org or right.org,\n        source="+".join(sources),\n    )\n\n\ndef _entry_json(entry: AssetCatalogEntry) -> dict:\n''',
)

picker = PY / "fresnica" / "tui" / "asset_picker.py"
replace(
    picker,
    '                "Recommended assets are cached locally. Full issuer identity remains authoritative.",\n',
    '                "Curated: Lobstr + Soroswap + StellarExpert · cached locally · full issuer identity remains authoritative.",\n',
)
replace(
    picker,
    '            entries = store.cached(record.network)\n',
    '            getter = getattr(store, "cached_curated", store.cached)\n            entries = getter(record.network)\n',
)
replace(
    picker,
    '            entries = store.recommended(record.network, limit=30, refresh=True)\n',
    '            getter = getattr(store, "curated", store.recommended)\n            entries = getter(record.network, limit=50, refresh=True)\n',
)
replace(
    picker,
    '            status.update(f"{len(entries)} recommended assets · R refresh · full issuer identity retained")\n',
    '            status.update(f"{len(entries)} curated assets · Lobstr + Soroswap + StellarExpert · R refresh")\n',
)
replace(
    picker,
    '            status.update("No recommended assets cached · enter a full asset identity below")\n',
    '            status.update("No curated assets cached · enter a full asset identity below")\n',
)

# Tests: support catalogue GETs without query params and prove three-source merge.
test_catalog = PY / "tests" / "test_asset_catalog.py"
replace(test_catalog, 'from fresnica.asset_catalog import AssetCatalogService\n', 'from fresnica.asset_catalog import ASSET_LIST_CATALOGUE, AssetCatalogService\n')
replace(test_catalog, '    def get(self, url, params, timeout):\n', '    def get(self, url, params=None, timeout=None):\n')
with test_catalog.open("a", encoding="utf-8") as handle:
    handle.write('''\n\nclass MultiSession:\n    def __init__(self, payloads):\n        self.payloads = payloads\n        self.calls = []\n\n    def get(self, url, params=None, timeout=None):\n        self.calls.append((url, params, timeout))\n        return Response(self.payloads[url])\n\n\ndef test_curated_catalog_merges_lobstr_soroswap_and_stellarexpert(tmp_path):\n    usdc = Keypair.random().public_key\n    aqua = Keypair.random().public_key\n    xrp = Keypair.random().public_key\n    lobstr_url = "https://lists.example/lobstr.json"\n    soroswap_url = "https://lists.example/soroswap.json"\n    expert_url = "https://lists.example/stellar-expert.json"\n    descriptors = [\n        {"name": "Lobstr Curated List", "provider": "UltraStellar", "url": lobstr_url},\n        {"name": "Soroswap List", "provider": "SoroswapFinance", "url": soroswap_url},\n        {"name": "StellarExpert Top 50", "provider": "StellarExpert", "url": expert_url},\n    ]\n    payloads = {\n        ASSET_LIST_CATALOGUE: descriptors,\n        lobstr_url: {"assets": [{"code": "USDC", "issuer": usdc, "name": "USD Coin", "domain": "circle.com"}]},\n        soroswap_url: {"assets": [{"code": "USDC", "issuer": usdc}, {"code": "AQUA", "issuer": aqua, "domain": "aqua.network"}]},\n        expert_url: {"assets": [{"code": "USDC", "issuer": usdc}, {"code": "XRP", "issuer": xrp, "domain": "fchain.io"}]},\n    }\n    path = tmp_path / "assets.json"\n    service = AssetCatalogService(path, session=MultiSession(payloads))\n\n    entries = service.curated("mainnet", limit=50)\n\n    assert [item.identity for item in entries] == [\n        "XLM", f"USDC:{usdc}", f"AQUA:{aqua}", f"XRP:{xrp}"\n    ]\n    assert entries[1].source == "lobstr+soroswap+stellar-expert"\n    assert entries[1].domain == "circle.com"\n    assert service.curated_path.name == "assets.curated.json"\n    cached = AssetCatalogService(path).cached_curated("mainnet")\n    assert [item.identity for item in cached] == [item.identity for item in entries]\n''')

# Existing TUI tests: new focus target and asset-management wording.
test_dex_ux = PY / "tests" / "test_tui_dex_market_ux.py"
replace(
    test_dex_ux,
    '            fills = app.screen.query_one("#dex-fills", DataTable)\n\n            bid_grid = bids.render()\n',
    '            fills = app.screen.query_one("#dex-fills", DataTable)\n            offers = app.screen.query_one("#dex-offers", DataTable)\n            assert app.screen.focused is offers\n\n            bid_grid = bids.render()\n',
)

test_trust = PY / "tests" / "test_tui_trustlines.py"
replace(
    test_trust,
    '            table = app.screen.query_one("#trustlines", DataTable)\n            assert table.row_count == 1\n',
    '            assert str(app.screen.query_one("#trust-title", Static).render()) == "Manage Assets"\n            assert "E set limit" not in str(app.screen.query_one("#trust-status", Static).render())\n            table = app.screen.query_one("#trustlines", DataTable)\n            assert table.row_count == 1\n',
)

print("manage-assets patch applied")

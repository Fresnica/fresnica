"""Shared cached asset catalog for trustlines and DEX market selection."""

from dataclasses import dataclass
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re

import requests

from .errors import FresnicaError, NetworkError
from .models import Asset


STELLAR_EXPERT_ASSETS = "https://api.stellar.expert/explorer/public/asset"
ASSET_LIST_CATALOGUE = "https://stellar-asset-lists.github.io/index/"
REQUEST_TIMEOUT = 5
MAX_RECOMMENDED = 50
MAX_CURATED_PER_SOURCE = 50
_CURATED_SOURCE_ORDER = ("lobstr", "soroswap", "stellar-expert")
_ASSET_ID = re.compile(r"^(.+)-(G[A-Z2-7]{55})-\d+$")


class AssetCatalogError(FresnicaError):
    pass


@dataclass(frozen=True)
class AssetCatalogEntry:
    asset: Asset
    domain: str | None = None
    name: str | None = None
    org: str | None = None
    source: str = "stellar-expert"

    @property
    def identity(self) -> str:
        if self.asset.is_native:
            return "XLM"
        return f"{self.asset.code}:{self.asset.issuer}"


class AssetCatalogService:
    """Cache-first recommended assets with manual identity as the fallback.

    Public recommendations are mainnet-only. Testnet/private-network assets are
    still supported by entering an exact CODE:G... identity in the picker.
    """

    def __init__(
        self,
        path: str | Path,
        session=None,
        timeout: int = REQUEST_TIMEOUT,
    ):
        self.path = Path(path).expanduser()
        self.session = session or requests.Session()
        self.timeout = timeout

    def cached(self, network: str) -> list[AssetCatalogEntry]:
        return self._read_cache(self.path, network)

    @property
    def curated_path(self) -> Path:
        suffix = self.path.suffix or ".json"
        return self.path.with_name(f"{self.path.stem}.curated{suffix}")

    def cached_curated(self, network: str) -> list[AssetCatalogEntry]:
        return self._read_cache(self.curated_path, network)

    def curated(
        self,
        network: str,
        limit: int = MAX_CURATED_PER_SOURCE,
        refresh: bool = True,
    ) -> list[AssetCatalogEntry]:
        limit = max(1, min(int(limit), MAX_CURATED_PER_SOURCE))
        cached = self.cached_curated(network)
        if network != "mainnet" or not refresh:
            return cached
        try:
            fresh, failures = self._fetch_curated(limit)
        except (requests.RequestException, ValueError, NetworkError):
            return cached
        if failures and len(cached) > 1:
            return cached
        if fresh:
            self._save_to(self.curated_path, fresh)
            return [AssetCatalogEntry(Asset("XLM"), source="native"), *fresh]
        return cached

    def recommended(
        self,
        network: str,
        limit: int = 30,
        refresh: bool = True,
    ) -> list[AssetCatalogEntry]:
        limit = max(1, min(int(limit), MAX_RECOMMENDED))
        cached = self.cached(network)
        if network != "mainnet" or not refresh:
            return cached[: limit + 1]
        try:
            fresh = self._fetch_ranked(limit)
        except (requests.RequestException, ValueError, NetworkError):
            return cached[: limit + 1]
        if fresh:
            self._save(fresh)
            return [AssetCatalogEntry(Asset("XLM"), source="native"), *fresh][: limit + 1]
        return cached[: limit + 1]

    def _read_cache(self, path: Path, network: str) -> list[AssetCatalogEntry]:
        if network != "mainnet":
            return [AssetCatalogEntry(Asset("XLM"), source="native")]
        try:
            raw = json.loads(path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return [AssetCatalogEntry(Asset("XLM"), source="native")]
        except (OSError, ValueError, TypeError) as exc:
            raise AssetCatalogError(f"Unable to read asset catalog: {path}") from exc
        if not isinstance(raw, dict) or raw.get("version") != 1:
            raise AssetCatalogError("Asset catalog cache is malformed")
        entries = [AssetCatalogEntry(Asset("XLM"), source="native")]
        seen = {entries[0].asset}
        for item in raw.get("assets", []):
            parsed = _entry_from_json(item)
            if parsed is not None and parsed.asset not in seen:
                entries.append(parsed)
                seen.add(parsed.asset)
        return entries

    def _fetch_curated(self, limit: int) -> tuple[list[AssetCatalogEntry], int]:
        response = self.session.get(ASSET_LIST_CATALOGUE, params=None, timeout=self.timeout)
        response.raise_for_status()
        descriptors = response.json()
        if not isinstance(descriptors, list):
            raise NetworkError("Asset-list catalogue response is malformed")

        selected: dict[str, dict] = {}
        for descriptor in descriptors:
            if not isinstance(descriptor, dict):
                continue
            source = _curated_source(descriptor)
            url = descriptor.get("url")
            if source and isinstance(url, str) and url:
                selected[source] = descriptor

        entries: list[AssetCatalogEntry] = []
        positions: dict[Asset, int] = {}
        failures = 0
        for source in _CURATED_SOURCE_ORDER:
            descriptor = selected.get(source)
            if descriptor is None:
                failures += 1
                continue
            try:
                list_response = self.session.get(
                    str(descriptor["url"]), params=None, timeout=self.timeout
                )
                list_response.raise_for_status()
                payload = list_response.json()
                assets = payload.get("assets", []) if isinstance(payload, dict) else []
                if not isinstance(assets, list):
                    raise NetworkError("Curated asset-list response is malformed")
            except (requests.RequestException, ValueError, NetworkError):
                failures += 1
                continue
            for raw in assets[:limit]:
                entry = _entry_from_sep42(raw, source)
                if entry is None:
                    continue
                position = positions.get(entry.asset)
                if position is None:
                    positions[entry.asset] = len(entries)
                    entries.append(entry)
                else:
                    entries[position] = _merge_entries(entries[position], entry)
        return entries, failures

    def _fetch_ranked(self, limit: int) -> list[AssetCatalogEntry]:
        response = self.session.get(
            STELLAR_EXPERT_ASSETS,
            params={
                "sort": "rating",
                "order": "desc",
                "limit": str(limit),
            },
            timeout=self.timeout,
        )
        response.raise_for_status()
        payload = response.json()
        if not isinstance(payload, dict):
            raise NetworkError("Ranked asset catalog response is malformed")
        records = payload.get("_embedded", {}).get("records", [])
        if not isinstance(records, list):
            raise NetworkError("Ranked asset catalog response is malformed")
        entries: list[AssetCatalogEntry] = []
        seen: set[Asset] = set()
        for raw in records:
            entry = _entry_from_stellar_expert(raw)
            if entry is None or entry.asset in seen:
                continue
            seen.add(entry.asset)
            entries.append(entry)
        return entries

    def _save(self, entries: list[AssetCatalogEntry]) -> None:
        self._save_to(self.path, entries)

    def _save_to(self, path: Path, entries: list[AssetCatalogEntry]) -> None:
        temporary = path.with_suffix(path.suffix + ".tmp")
        payload = {
            "version": 1,
            "saved_at": datetime.now(timezone.utc).isoformat(),
            "assets": [_entry_json(item) for item in entries],
        }
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            temporary.write_text(
                json.dumps(payload, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            try:
                os.chmod(temporary, 0o600)
            except OSError:
                pass
            os.replace(temporary, path)
        except OSError as exc:
            temporary.unlink(missing_ok=True)
            raise AssetCatalogError(f"Unable to write asset catalog: {path}") from exc


def _entry_from_stellar_expert(raw) -> AssetCatalogEntry | None:
    if not isinstance(raw, dict):
        return None
    match = _ASSET_ID.match(str(raw.get("asset") or ""))
    if match is None:
        return None
    try:
        asset = Asset(match.group(1), match.group(2))
    except (FresnicaError, ValueError):
        return None
    toml = raw.get("tomlInfo") if isinstance(raw.get("tomlInfo"), dict) else {}
    return AssetCatalogEntry(
        asset=asset,
        domain=_optional_text(raw.get("domain")),
        name=_optional_text(toml.get("name")),
        org=_optional_text(toml.get("orgName")),
    )


def _curated_source(descriptor: dict) -> str | None:
    text = f"{descriptor.get('name', '')} {descriptor.get('provider', '')}".lower()
    if "lobstr" in text:
        return "lobstr"
    if "soroswap" in text:
        return "soroswap"
    if "stellarexpert" in text or "stellar expert" in text:
        return "stellar-expert"
    return None


def _entry_from_sep42(raw, source: str) -> AssetCatalogEntry | None:
    if not isinstance(raw, dict):
        return None
    code = raw.get("code")
    issuer = raw.get("issuer")
    if not isinstance(code, str) or not isinstance(issuer, str):
        return None
    try:
        asset = Asset(code, issuer)
    except (FresnicaError, ValueError):
        return None
    return AssetCatalogEntry(
        asset=asset,
        domain=_optional_text(raw.get("domain")),
        name=_optional_text(raw.get("name")),
        org=_optional_text(raw.get("org")),
        source=source,
    )


def _merge_entries(left: AssetCatalogEntry, right: AssetCatalogEntry) -> AssetCatalogEntry:
    sources = []
    for source in (*left.source.split("+"), *right.source.split("+")):
        if source and source not in sources:
            sources.append(source)
    return AssetCatalogEntry(
        asset=left.asset,
        domain=left.domain or right.domain,
        name=left.name or right.name,
        org=left.org or right.org,
        source="+".join(sources),
    )


def _entry_json(entry: AssetCatalogEntry) -> dict:
    return {
        "code": entry.asset.code,
        "issuer": entry.asset.issuer,
        "domain": entry.domain,
        "name": entry.name,
        "org": entry.org,
        "source": entry.source,
    }


def _entry_from_json(raw) -> AssetCatalogEntry | None:
    if not isinstance(raw, dict):
        return None
    try:
        asset = Asset(str(raw["code"]), str(raw["issuer"]))
    except (KeyError, FresnicaError, ValueError):
        return None
    return AssetCatalogEntry(
        asset=asset,
        domain=_optional_text(raw.get("domain")),
        name=_optional_text(raw.get("name")),
        org=_optional_text(raw.get("org")),
        source=_optional_text(raw.get("source")) or "cache",
    )


def _optional_text(value) -> str | None:
    if not isinstance(value, str):
        return None
    value = value.strip()
    return value or None

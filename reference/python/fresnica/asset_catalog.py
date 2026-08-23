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
REQUEST_TIMEOUT = 5
MAX_RECOMMENDED = 50
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
        if network != "mainnet":
            return [AssetCatalogEntry(Asset("XLM"), source="native")]
        try:
            raw = json.loads(self.path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return [AssetCatalogEntry(Asset("XLM"), source="native")]
        except (OSError, ValueError, TypeError) as exc:
            raise AssetCatalogError(f"Unable to read asset catalog: {self.path}") from exc
        if not isinstance(raw, dict) or raw.get("version") != 1:
            raise AssetCatalogError("Asset catalog cache is malformed")
        entries = [AssetCatalogEntry(Asset("XLM"), source="native")]
        for item in raw.get("assets", []):
            parsed = _entry_from_json(item)
            if parsed is not None and parsed.asset not in {entry.asset for entry in entries}:
                entries.append(parsed)
        return entries

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
        temporary = self.path.with_suffix(self.path.suffix + ".tmp")
        payload = {
            "version": 1,
            "saved_at": datetime.now(timezone.utc).isoformat(),
            "assets": [_entry_json(item) for item in entries],
        }
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary.write_text(
                json.dumps(payload, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            try:
                os.chmod(temporary, 0o600)
            except OSError:
                pass
            os.replace(temporary, self.path)
        except OSError as exc:
            temporary.unlink(missing_ok=True)
            raise AssetCatalogError(f"Unable to write asset catalog: {self.path}") from exc


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

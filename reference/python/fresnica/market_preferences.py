"""Per-wallet DEX market favorites and recents."""

from dataclasses import dataclass
import json
import os
from pathlib import Path

from .errors import FresnicaError
from .models import Asset, MarketPair


MAX_FAVORITES = 50
MAX_RECENTS = 12


class MarketPreferencesError(FresnicaError):
    pass


@dataclass(frozen=True)
class MarketPreferences:
    favorites: tuple[MarketPair, ...] = ()
    recents: tuple[MarketPair, ...] = ()


class MarketPreferencesStore:
    def __init__(self, path: str | Path):
        self.path = Path(path).expanduser()

    def get(self, network: str, address: str) -> MarketPreferences:
        scope = self._load().get(_scope(network, address), {})
        return MarketPreferences(
            favorites=tuple(_pairs(scope.get("favorites", []))),
            recents=tuple(_pairs(scope.get("recents", []))),
        )

    def touch(self, network: str, address: str, pair: MarketPair) -> MarketPreferences:
        data = self._load()
        key = _scope(network, address)
        scope = data.setdefault(key, {})
        recents = [item for item in _pairs(scope.get("recents", [])) if item != pair]
        recents.insert(0, pair)
        scope["recents"] = [_pair_json(item) for item in recents[:MAX_RECENTS]]
        self._save(data)
        return self.get(network, address)

    def toggle_favorite(
        self,
        network: str,
        address: str,
        pair: MarketPair,
    ) -> MarketPreferences:
        data = self._load()
        key = _scope(network, address)
        scope = data.setdefault(key, {})
        favorites = _pairs(scope.get("favorites", []))
        if pair in favorites:
            favorites = [item for item in favorites if item != pair]
        else:
            favorites.insert(0, pair)
        scope["favorites"] = [_pair_json(item) for item in favorites[:MAX_FAVORITES]]
        self._save(data)
        return self.get(network, address)

    def _load(self) -> dict:
        if not self.path.exists():
            return {}
        try:
            raw = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError, TypeError) as exc:
            raise MarketPreferencesError(f"Unable to read DEX market preferences: {self.path}") from exc
        if not isinstance(raw, dict):
            raise MarketPreferencesError("DEX market preferences are malformed")
        scopes = raw.get("scopes", {})
        if not isinstance(scopes, dict):
            raise MarketPreferencesError("DEX market preferences are malformed")
        return scopes

    def _save(self, scopes: dict) -> None:
        temporary = self.path.with_suffix(self.path.suffix + ".tmp")
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary.write_text(
                json.dumps({"version": 1, "scopes": scopes}, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            try:
                os.chmod(temporary, 0o600)
            except OSError:
                pass
            os.replace(temporary, self.path)
        except OSError as exc:
            temporary.unlink(missing_ok=True)
            raise MarketPreferencesError(f"Unable to write DEX market preferences: {self.path}") from exc


def asset_identity(asset: Asset) -> str:
    if asset.is_native:
        return "XLM"
    if asset.is_liquidity_pool:
        raise ValueError("Liquidity-pool shares cannot be a DEX market asset")
    return f"{asset.code}:{asset.issuer}"


def pair_identity(pair: MarketPair) -> str:
    return f"{asset_identity(pair.base)}>{asset_identity(pair.counter)}"


def _scope(network: str, address: str) -> str:
    return f"{network.lower()}:{address}"


def _pair_json(pair: MarketPair) -> dict:
    return {"base": asset_identity(pair.base), "counter": asset_identity(pair.counter)}


def _pairs(values) -> list[MarketPair]:
    if not isinstance(values, list):
        return []
    result: list[MarketPair] = []
    for raw in values:
        if not isinstance(raw, dict):
            continue
        try:
            pair = MarketPair(
                base=Asset.parse(str(raw["base"])),
                counter=Asset.parse(str(raw["counter"])),
            )
        except (KeyError, FresnicaError, ValueError):
            continue
        if pair.base == pair.counter or pair in result:
            continue
        result.append(pair)
    return result

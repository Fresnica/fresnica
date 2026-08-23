"""Persistent cache for explicitly discovered anchor capabilities."""

from datetime import datetime, timezone
import json
import os
from pathlib import Path

from .anchor_service import AnchorCapabilities
from .errors import FresnicaError
from .models import Asset


class AnchorCapabilitiesCacheError(FresnicaError):
    pass


class AnchorCapabilitiesStore:
    """Persist discovered SEP metadata by exact issued asset and home domain.

    Discovery remains explicit. Once discovered, reopening Asset Details can use
    the cached capabilities without another stellar.toml or /info request. The
    user can still press A to refresh the cache manually.
    """

    def __init__(self, path: str | Path):
        self.path = Path(path).expanduser()

    def get(self, asset: Asset, domain: str) -> AnchorCapabilities | None:
        key = _key(asset, domain)
        if key is None:
            return None
        raw = self._load().get(key)
        if not isinstance(raw, dict):
            return None
        value = raw.get("capabilities")
        if not isinstance(value, dict):
            return None
        try:
            return AnchorCapabilities(
                domain=str(value["domain"]),
                sep6_url=_optional_text(value.get("sep6_url")),
                sep24_url=_optional_text(value.get("sep24_url")),
                web_auth_url=_optional_text(value.get("web_auth_url")),
                signing_key=_optional_text(value.get("signing_key")),
                kyc_url=_optional_text(value.get("kyc_url")),
                direct_payment_url=_optional_text(value.get("direct_payment_url")),
                sep6_deposit=bool(value.get("sep6_deposit", False)),
                sep6_withdraw=bool(value.get("sep6_withdraw", False)),
                sep24_deposit=bool(value.get("sep24_deposit", False)),
                sep24_withdraw=bool(value.get("sep24_withdraw", False)),
                warnings=tuple(
                    str(item) for item in value.get("warnings", []) if isinstance(item, str)
                ),
            )
        except (KeyError, TypeError, ValueError):
            return None

    def put(self, asset: Asset, capabilities: AnchorCapabilities) -> None:
        key = _key(asset, capabilities.domain)
        if key is None:
            return
        entries = self._load()
        entries[key] = {
            "saved_at": datetime.now(timezone.utc).isoformat(),
            "capabilities": {
                "domain": capabilities.domain,
                "sep6_url": capabilities.sep6_url,
                "sep24_url": capabilities.sep24_url,
                "web_auth_url": capabilities.web_auth_url,
                "signing_key": capabilities.signing_key,
                "kyc_url": capabilities.kyc_url,
                "direct_payment_url": capabilities.direct_payment_url,
                "sep6_deposit": capabilities.sep6_deposit,
                "sep6_withdraw": capabilities.sep6_withdraw,
                "sep24_deposit": capabilities.sep24_deposit,
                "sep24_withdraw": capabilities.sep24_withdraw,
                "warnings": list(capabilities.warnings),
            },
        }
        self._save(entries)

    def _load(self) -> dict:
        if not self.path.exists():
            return {}
        try:
            raw = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError, TypeError) as exc:
            raise AnchorCapabilitiesCacheError(
                f"Unable to read anchor capability cache: {self.path}"
            ) from exc
        if not isinstance(raw, dict) or raw.get("version") != 1:
            raise AnchorCapabilitiesCacheError("Anchor capability cache is malformed")
        entries = raw.get("entries", {})
        if not isinstance(entries, dict):
            raise AnchorCapabilitiesCacheError("Anchor capability cache is malformed")
        return entries

    def _save(self, entries: dict) -> None:
        temporary = self.path.with_suffix(self.path.suffix + ".tmp")
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary.write_text(
                json.dumps(
                    {"version": 1, "entries": entries},
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
            try:
                os.chmod(temporary, 0o600)
            except OSError:
                pass
            os.replace(temporary, self.path)
        except OSError as exc:
            temporary.unlink(missing_ok=True)
            raise AnchorCapabilitiesCacheError(
                f"Unable to write anchor capability cache: {self.path}"
            ) from exc


def _key(asset: Asset, domain: str) -> str | None:
    if asset.is_native or asset.is_liquidity_pool or not asset.issuer:
        return None
    host = str(domain or "").strip().lower().rstrip(".")
    if not host:
        return None
    return f"{asset.code}:{asset.issuer}@{host}"


def _optional_text(value) -> str | None:
    if not isinstance(value, str):
        return None
    value = value.strip()
    return value or None

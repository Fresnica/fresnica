"""Lazy SEP-1/6/24 capability discovery for issued-asset details."""

from dataclasses import dataclass, field
import tomllib
from urllib.parse import urljoin

import requests

from .errors import NetworkError
from .models import Asset


MAX_STELLAR_TOML_BYTES = 1_000_000
REQUEST_TIMEOUT = 5


@dataclass(frozen=True)
class AnchorCapabilities:
    domain: str
    sep6_url: str | None = None
    sep24_url: str | None = None
    web_auth_url: str | None = None
    kyc_url: str | None = None
    direct_payment_url: str | None = None
    sep6_deposit: bool = False
    sep6_withdraw: bool = False
    sep24_deposit: bool = False
    sep24_withdraw: bool = False
    warnings: tuple[str, ...] = field(default_factory=tuple)


class AnchorService:
    """Discovers anchor infrastructure only when the user opens asset details."""

    def __init__(self, session=None, timeout: int = REQUEST_TIMEOUT):
        self.session = session or requests.Session()
        self.timeout = timeout

    def discover(self, asset: Asset, home_domain: str) -> AnchorCapabilities:
        if asset.is_native or asset.is_liquidity_pool or not asset.issuer:
            return AnchorCapabilities(domain=home_domain)
        domain = _domain(home_domain)
        toml = self._stellar_toml(domain)
        if not _currency_matches(toml, asset):
            return AnchorCapabilities(
                domain=domain,
                warnings=("stellar.toml does not list this exact asset",),
            )

        sep6 = _text(toml.get("TRANSFER_SERVER"))
        sep24 = _text(toml.get("TRANSFER_SERVER_SEP0024"))
        warnings: list[str] = []
        sep6_deposit = sep6_withdraw = False
        sep24_deposit = sep24_withdraw = False

        if sep6:
            try:
                info = self._json(urljoin(sep6.rstrip("/") + "/", "info"))
                sep6_deposit = _asset_enabled(info.get("deposit"), asset.code)
                sep6_withdraw = _asset_enabled(info.get("withdraw"), asset.code)
            except NetworkError as exc:
                warnings.append(f"SEP-6 /info unavailable: {exc}")

        if sep24:
            try:
                info = self._json(urljoin(sep24.rstrip("/") + "/", "info"))
                sep24_deposit = _asset_enabled(info.get("deposit"), asset.code)
                sep24_withdraw = _asset_enabled(info.get("withdraw"), asset.code)
            except NetworkError as exc:
                warnings.append(f"SEP-24 /info unavailable: {exc}")

        return AnchorCapabilities(
            domain=domain,
            sep6_url=sep6,
            sep24_url=sep24,
            web_auth_url=_text(toml.get("WEB_AUTH_ENDPOINT")),
            kyc_url=_text(toml.get("KYC_SERVER")),
            direct_payment_url=_text(toml.get("DIRECT_PAYMENT_SERVER")),
            sep6_deposit=sep6_deposit,
            sep6_withdraw=sep6_withdraw,
            sep24_deposit=sep24_deposit,
            sep24_withdraw=sep24_withdraw,
            warnings=tuple(warnings),
        )

    def _stellar_toml(self, domain: str) -> dict:
        url = f"https://{domain}/.well-known/stellar.toml"
        try:
            response = self.session.get(url, timeout=self.timeout)
            response.raise_for_status()
        except requests.RequestException as exc:
            raise NetworkError(f"Unable to load stellar.toml from {domain}") from exc
        content = response.content
        if len(content) > MAX_STELLAR_TOML_BYTES:
            raise NetworkError(f"stellar.toml from {domain} is too large")
        try:
            return tomllib.loads(content.decode("utf-8"))
        except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
            raise NetworkError(f"Invalid stellar.toml from {domain}") from exc

    def _json(self, url: str) -> dict:
        try:
            response = self.session.get(url, timeout=self.timeout)
            response.raise_for_status()
            value = response.json()
        except (requests.RequestException, ValueError) as exc:
            raise NetworkError(f"Unable to load anchor info from {url}") from exc
        if not isinstance(value, dict):
            raise NetworkError(f"Anchor info from {url} is malformed")
        return value


def _domain(value: str) -> str:
    domain = str(value or "").strip().lower().rstrip(".")
    if not domain or "://" in domain or "/" in domain or "\\" in domain:
        raise NetworkError("Issuer home_domain is not a valid host name")
    return domain


def _text(value) -> str | None:
    return str(value).strip() if isinstance(value, str) and value.strip() else None


def _currency_matches(toml: dict, asset: Asset) -> bool:
    currencies = toml.get("CURRENCIES")
    if not isinstance(currencies, list):
        return True
    for item in currencies:
        if not isinstance(item, dict):
            continue
        if str(item.get("code", "")).upper() != asset.code.upper():
            continue
        issuer = item.get("issuer")
        if issuer is None or str(issuer) == asset.issuer:
            return True
    return False


def _asset_enabled(section, code: str) -> bool:
    if not isinstance(section, dict):
        return False
    value = section.get(code)
    if not isinstance(value, dict):
        value = section.get(code.upper()) or section.get(code.lower())
    if not isinstance(value, dict):
        return False
    return bool(value.get("enabled", True))

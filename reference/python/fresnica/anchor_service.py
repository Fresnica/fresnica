"""Explicit SEP-1/10/6/24 discovery and interactive anchor transfer support."""

from dataclasses import dataclass, field
import tomllib
from urllib.parse import urljoin, urlparse

import requests
from stellar_sdk.sep.exceptions import InvalidSep10ChallengeError
from stellar_sdk.sep.stellar_web_authentication import read_challenge_transaction

from .errors import FresnicaError, NetworkError
from .models import Asset


MAX_STELLAR_TOML_BYTES = 1_000_000
REQUEST_TIMEOUT = 5


class AnchorError(FresnicaError):
    pass


@dataclass(frozen=True)
class AnchorCapabilities:
    domain: str
    sep6_url: str | None = None
    sep24_url: str | None = None
    web_auth_url: str | None = None
    signing_key: str | None = None
    kyc_url: str | None = None
    direct_payment_url: str | None = None
    sep6_deposit: bool = False
    sep6_withdraw: bool = False
    sep24_deposit: bool = False
    sep24_withdraw: bool = False
    warnings: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class AnchorInteractiveTransfer:
    kind: str
    url: str
    transaction_id: str | None = None


class AnchorService:
    """Anchor infrastructure accessed only after an explicit user action."""

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

        sep6 = _endpoint(toml.get("TRANSFER_SERVER"))
        sep24 = _endpoint(toml.get("TRANSFER_SERVER_SEP0024"))
        web_auth = _endpoint(toml.get("WEB_AUTH_ENDPOINT"))
        signing_key = _text(toml.get("SIGNING_KEY"))
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

        if (sep24_deposit or sep24_withdraw) and (not web_auth or not signing_key):
            warnings.append("SEP-24 advertised without complete SEP-10 authentication metadata")

        return AnchorCapabilities(
            domain=domain,
            sep6_url=sep6,
            sep24_url=sep24,
            web_auth_url=web_auth,
            signing_key=signing_key,
            kyc_url=_endpoint(toml.get("KYC_SERVER")),
            direct_payment_url=_endpoint(toml.get("DIRECT_PAYMENT_SERVER")),
            sep6_deposit=sep6_deposit,
            sep6_withdraw=sep6_withdraw,
            sep24_deposit=sep24_deposit,
            sep24_withdraw=sep24_withdraw,
            warnings=tuple(warnings),
        )

    def start_sep24(
        self,
        wallet,
        asset: Asset,
        capabilities: AnchorCapabilities,
        kind: str,
        network_passphrase: str,
    ) -> AnchorInteractiveTransfer:
        if kind not in {"deposit", "withdraw"}:
            raise ValueError(f"Unsupported anchor transfer kind: {kind}")
        enabled = (
            capabilities.sep24_deposit if kind == "deposit" else capabilities.sep24_withdraw
        )
        if not enabled or not capabilities.sep24_url:
            raise AnchorError(f"SEP-24 {kind} is not available for {asset.code}")
        if not capabilities.web_auth_url or not capabilities.signing_key:
            raise AnchorError("Anchor SEP-24 flow is missing SEP-10 authentication metadata")
        if asset.is_native or asset.is_liquidity_pool or not asset.issuer:
            raise AnchorError("SEP-24 asset must be an issued Stellar asset")

        token = self._authenticate_sep10(
            wallet,
            capabilities,
            network_passphrase,
        )
        endpoint = urljoin(
            capabilities.sep24_url.rstrip("/") + "/",
            f"transactions/{kind}/interactive",
        )
        payload = self._post_json(
            endpoint,
            data={
                "asset_code": asset.code,
                "account": wallet.address(),
            },
            headers={"Authorization": f"Bearer {token}"},
        )
        transfer_type = str(payload.get("type") or "")
        url = _endpoint(payload.get("url"))
        if transfer_type != "interactive_customer_info_needed" or not url:
            raise AnchorError("Anchor returned an invalid SEP-24 interactive response")
        return AnchorInteractiveTransfer(
            kind=kind,
            url=url,
            transaction_id=_text(payload.get("id")),
        )

    def _authenticate_sep10(
        self,
        wallet,
        capabilities: AnchorCapabilities,
        network_passphrase: str,
    ) -> str:
        assert capabilities.web_auth_url is not None
        assert capabilities.signing_key is not None
        endpoint = capabilities.web_auth_url
        try:
            response = self.session.get(
                endpoint,
                params={"account": wallet.address()},
                timeout=self.timeout,
            )
            response.raise_for_status()
            challenge_payload = response.json()
        except (requests.RequestException, ValueError) as exc:
            raise NetworkError("Unable to request SEP-10 authentication challenge") from exc
        if not isinstance(challenge_payload, dict):
            raise AnchorError("Anchor SEP-10 challenge response is malformed")
        challenge_xdr = _text(challenge_payload.get("transaction"))
        if not challenge_xdr:
            raise AnchorError("Anchor SEP-10 challenge response has no transaction")

        web_auth_domain = _url_host(endpoint)
        try:
            parsed = read_challenge_transaction(
                challenge_xdr,
                capabilities.signing_key,
                capabilities.domain,
                web_auth_domain,
                network_passphrase,
            )
        except (InvalidSep10ChallengeError, ValueError) as exc:
            raise AnchorError("Anchor SEP-10 challenge failed verification") from exc
        if parsed.client_account_id != wallet.address():
            raise AnchorError("Anchor SEP-10 challenge targets a different account")

        wallet.sign(parsed.transaction)
        auth_payload = self._post_json(
            endpoint,
            json_body={"transaction": parsed.transaction.to_xdr()},
        )
        token = _text(auth_payload.get("token"))
        if not token:
            raise AnchorError("Anchor SEP-10 authentication returned no token")
        return token

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

    def _post_json(self, url: str, *, data=None, json_body=None, headers=None) -> dict:
        try:
            response = self.session.post(
                url,
                data=data,
                json=json_body,
                headers=headers,
                timeout=self.timeout,
            )
            response.raise_for_status()
            value = response.json()
        except (requests.RequestException, ValueError) as exc:
            raise NetworkError(f"Unable to call anchor endpoint {url}") from exc
        if not isinstance(value, dict):
            raise AnchorError(f"Anchor response from {url} is malformed")
        return value


def _domain(value: str) -> str:
    domain = str(value or "").strip().lower().rstrip(".")
    if not domain or "://" in domain or "/" in domain or "\\" in domain:
        raise NetworkError("Issuer home_domain is not a valid host name")
    return domain


def _endpoint(value) -> str | None:
    text = _text(value)
    if text is None:
        return None
    parsed = urlparse(text)
    if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password:
        raise NetworkError("Anchor endpoint must be an HTTPS URL without embedded credentials")
    return text


def _url_host(value: str) -> str:
    parsed = urlparse(value)
    if not parsed.hostname:
        raise AnchorError("Anchor web authentication URL has no host")
    return parsed.hostname


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

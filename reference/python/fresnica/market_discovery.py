"""Best-effort public market suggestions for the DEX chooser."""

import re

import requests

from .models import Asset, MarketPair


STELLAR_EXPERT_ASSETS = "https://api.stellar.expert/explorer/public/asset"
REQUEST_TIMEOUT = 5
_ASSET_ID = re.compile(r"^(.+)-(G[A-Z2-7]{55})-\d+$")


class MarketDiscoveryService:
    def __init__(self, session=None, timeout: int = REQUEST_TIMEOUT):
        self.session = session or requests.Session()
        self.timeout = timeout

    def popular_pairs(self, network: str, limit: int = 10) -> list[MarketPair]:
        if network != "mainnet":
            return []
        response = self.session.get(
            STELLAR_EXPERT_ASSETS,
            params={
                "sort": "volume7d",
                "order": "desc",
                "limit": str(limit + 6),
            },
            timeout=self.timeout,
        )
        response.raise_for_status()
        payload = response.json()
        records = payload.get("_embedded", {}).get("records", []) if isinstance(payload, dict) else []
        assets = []
        for raw in records:
            asset = _asset(raw)
            if asset is not None and asset not in assets:
                assets.append(asset)

        usdc = next(
            (
                item
                for item, domain in assets
                if item.code.upper() == "USDC" and "circle.com" in domain.lower()
            ),
            None,
        )
        pairs: list[MarketPair] = []
        if usdc is not None:
            pairs.append(MarketPair(Asset("XLM"), usdc))
        for asset, _domain in assets:
            if usdc is not None and asset == usdc:
                continue
            pair = MarketPair(asset, Asset("XLM"))
            if pair not in pairs:
                pairs.append(pair)
            if usdc is not None and asset.code.upper() in {"XRP", "YXLM", "AQUA"}:
                pair = MarketPair(asset, usdc)
                if pair not in pairs:
                    pairs.append(pair)
            if len(pairs) >= limit:
                break
        return pairs[:limit]


def _asset(raw) -> tuple[Asset, str] | None:
    if not isinstance(raw, dict):
        return None
    match = _ASSET_ID.match(str(raw.get("asset") or ""))
    if match is None:
        return None
    domain = str(raw.get("domain") or "").strip()
    if not domain:
        return None
    return Asset(match.group(1), match.group(2)), domain

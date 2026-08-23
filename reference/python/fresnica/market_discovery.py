"""Fex-style popular market derivation from the shared asset catalog."""

from .asset_catalog import AssetCatalogEntry, AssetCatalogService
from .models import Asset, MarketPair


class MarketDiscoveryService:
    def __init__(self, catalog: AssetCatalogService):
        self.catalog = catalog

    def popular_pairs(
        self,
        network: str,
        limit: int = 12,
        held_assets: list[Asset] | tuple[Asset, ...] = (),
        refresh: bool = True,
    ) -> list[MarketPair]:
        if network != "mainnet":
            return []
        entries = self.catalog.recommended(
            network,
            limit=max(limit + 6, 18),
            refresh=refresh,
        )
        pairs = popular_pairs_from_assets(entries, limit=limit)
        return order_pairs_by_held_assets(pairs, held_assets)


def popular_pairs_from_assets(
    entries: list[AssetCatalogEntry] | tuple[AssetCatalogEntry, ...],
    limit: int = 12,
) -> list[MarketPair]:
    """Match Fex's source-order popular pair construction."""
    issued = [
        item
        for item in entries
        if not item.asset.is_native and not item.asset.is_liquidity_pool and item.asset.issuer
    ]
    usdc = next(
        (
            item.asset
            for item in issued
            if item.asset.code.upper() == "USDC"
            and item.domain
            and "circle.com" in item.domain.lower()
        ),
        None,
    )

    native = Asset("XLM")
    pairs: list[MarketPair] = []
    if usdc is not None:
        pairs.append(MarketPair(native, usdc))

    for entry in issued:
        asset = entry.asset
        if usdc is not None and asset == usdc:
            continue
        _append_unique(pairs, MarketPair(asset, native))
        if (
            usdc is not None
            and len(pairs) < limit
            and asset.code.upper() in {"XRP", "YXLM", "AQUA"}
        ):
            _append_unique(pairs, MarketPair(asset, usdc))
        if len(pairs) >= limit:
            break
    return pairs[:limit]


def order_pairs_by_held_assets(
    pairs: list[MarketPair] | tuple[MarketPair, ...],
    held_assets: list[Asset] | tuple[Asset, ...],
) -> list[MarketPair]:
    """Stable Fex ordering: 2 held legs, then 1, then 0."""
    if not held_assets:
        return list(pairs)
    held = set(held_assets)
    return sorted(
        pairs,
        key=lambda pair: -int(pair.base in held) - int(pair.counter in held),
    )


def _append_unique(items: list[MarketPair], pair: MarketPair) -> None:
    if pair.base != pair.counter and pair not in items:
        items.append(pair)

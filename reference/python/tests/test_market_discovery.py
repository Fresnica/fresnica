from stellar_sdk import Keypair

from fresnica.asset_catalog import AssetCatalogEntry
from fresnica.market_discovery import MarketDiscoveryService, order_pairs_by_held_assets
from fresnica.models import Asset, MarketPair


class Catalog:
    def __init__(self, entries):
        self.entries = entries
        self.calls = []

    def recommended(self, network, limit=30, refresh=True):
        self.calls.append((network, limit, refresh))
        return self.entries


def test_popular_markets_follow_shared_catalog_and_verified_usdc_domain():
    usdc = Keypair.random().public_key
    xrp = Keypair.random().public_key
    aqua = Keypair.random().public_key
    catalog = Catalog(
        [
            AssetCatalogEntry(Asset("XLM"), source="native"),
            AssetCatalogEntry(Asset("USDC", usdc), domain="circle.com"),
            AssetCatalogEntry(Asset("XRP", xrp), domain="example.org"),
            AssetCatalogEntry(Asset("AQUA", aqua), domain="aqua.network"),
        ]
    )
    pairs = MarketDiscoveryService(catalog).popular_pairs("mainnet", limit=6)

    assert pairs[0] == MarketPair(Asset("XLM"), Asset("USDC", usdc))
    assert MarketPair(Asset("XRP", xrp), Asset("XLM")) in pairs
    assert MarketPair(Asset("XRP", xrp), Asset("USDC", usdc)) in pairs
    assert MarketPair(Asset("AQUA", aqua), Asset("XLM")) in pairs
    assert catalog.calls == [("mainnet", 18, True)]


def test_popular_markets_prioritize_pairs_with_more_held_legs_stably():
    usdc = Asset("USDC", Keypair.random().public_key)
    xrp = Asset("XRP", Keypair.random().public_key)
    aqua = Asset("AQUA", Keypair.random().public_key)
    xlm = Asset("XLM")
    pairs = [
        MarketPair(xlm, usdc),
        MarketPair(xrp, xlm),
        MarketPair(aqua, xlm),
        MarketPair(xrp, usdc),
    ]

    ordered = order_pairs_by_held_assets(pairs, [xrp, usdc])

    assert ordered == [
        MarketPair(xrp, usdc),
        MarketPair(xlm, usdc),
        MarketPair(xrp, xlm),
        MarketPair(aqua, xlm),
    ]


def test_popular_markets_do_not_call_catalog_on_testnet():
    catalog = Catalog([])
    assert MarketDiscoveryService(catalog).popular_pairs("testnet") == []
    assert catalog.calls == []

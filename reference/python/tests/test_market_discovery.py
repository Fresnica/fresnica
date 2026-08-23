from stellar_sdk import Keypair

from fresnica.market_discovery import MarketDiscoveryService
from fresnica.models import Asset, MarketPair


class Response:
    def __init__(self, payload):
        self.payload = payload

    def raise_for_status(self):
        return None

    def json(self):
        return self.payload


class Session:
    def __init__(self, payload):
        self.payload = payload
        self.calls = []

    def get(self, url, params, timeout):
        self.calls.append((url, params, timeout))
        return Response(self.payload)


def test_popular_markets_follow_ranked_assets_and_verified_usdc_domain():
    usdc = Keypair.random().public_key
    xrp = Keypair.random().public_key
    aqua = Keypair.random().public_key
    payload = {
        "_embedded": {
            "records": [
                {"asset": f"USDC-{usdc}-1", "domain": "circle.com"},
                {"asset": f"XRP-{xrp}-2", "domain": "example.org"},
                {"asset": f"AQUA-{aqua}-3", "domain": "aqua.network"},
            ]
        }
    }
    session = Session(payload)
    pairs = MarketDiscoveryService(session=session).popular_pairs("mainnet", limit=6)

    assert pairs[0] == MarketPair(Asset("XLM"), Asset("USDC", usdc))
    assert MarketPair(Asset("XRP", xrp), Asset("XLM")) in pairs
    assert MarketPair(Asset("XRP", xrp), Asset("USDC", usdc)) in pairs
    assert MarketPair(Asset("AQUA", aqua), Asset("XLM")) in pairs
    assert len(session.calls) == 1


def test_popular_markets_do_not_call_third_party_on_testnet():
    session = Session({})
    assert MarketDiscoveryService(session=session).popular_pairs("testnet") == []
    assert session.calls == []

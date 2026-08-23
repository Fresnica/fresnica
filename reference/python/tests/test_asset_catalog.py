import json

import requests
from stellar_sdk import Keypair

from fresnica.asset_catalog import ASSET_LIST_CATALOGUE, AssetCatalogService


class Response:
    def __init__(self, payload):
        self.payload = payload

    def raise_for_status(self):
        return None

    def json(self):
        return self.payload


class Session:
    def __init__(self, payload=None, error=None):
        self.payload = payload
        self.error = error
        self.calls = []

    def get(self, url, params=None, timeout=None):
        self.calls.append((url, params, timeout))
        if self.error is not None:
            raise self.error
        return Response(self.payload)


def test_ranked_asset_catalog_persists_full_identity_and_metadata(tmp_path):
    usdc = Keypair.random().public_key
    aqua = Keypair.random().public_key
    payload = {
        "_embedded": {
            "records": [
                {
                    "asset": f"USDC-{usdc}-1",
                    "domain": "circle.com",
                    "tomlInfo": {"name": "USD Coin", "orgName": "Circle"},
                },
                {
                    "asset": f"AQUA-{aqua}-2",
                    "domain": "aqua.network",
                    "tomlInfo": {"name": "Aquarius"},
                },
            ]
        }
    }
    path = tmp_path / "assets.json"
    service = AssetCatalogService(path, session=Session(payload))

    entries = service.recommended("mainnet", limit=10)

    assert entries[0].identity == "XLM"
    assert entries[1].identity == f"USDC:{usdc}"
    assert entries[1].domain == "circle.com"
    assert entries[1].name == "USD Coin"
    assert entries[2].identity == f"AQUA:{aqua}"
    saved = json.loads(path.read_text(encoding="utf-8"))
    assert saved["assets"][0]["issuer"] == usdc
    assert AssetCatalogService(path).cached("mainnet")[1].asset.issuer == usdc


def test_ranked_catalog_falls_back_to_cached_assets_on_network_failure(tmp_path):
    issuer = Keypair.random().public_key
    path = tmp_path / "assets.json"
    path.write_text(
        json.dumps(
            {
                "version": 1,
                "saved_at": "2026-08-23T00:00:00+00:00",
                "assets": [
                    {
                        "code": "USD",
                        "issuer": issuer,
                        "domain": "anchor.example",
                        "name": None,
                        "org": None,
                        "source": "stellar-expert",
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    service = AssetCatalogService(
        path,
        session=Session(error=requests.ConnectionError("offline")),
    )

    entries = service.recommended("mainnet", limit=10)

    assert [item.identity for item in entries] == ["XLM", f"USD:{issuer}"]


def test_testnet_catalog_never_fetches_public_mainnet_recommendations(tmp_path):
    session = Session({"_embedded": {"records": []}})
    service = AssetCatalogService(tmp_path / "assets.json", session=session)

    entries = service.recommended("testnet", limit=10)

    assert [item.identity for item in entries] == ["XLM"]
    assert session.calls == []


class MultiSession:
    def __init__(self, payloads):
        self.payloads = payloads
        self.calls = []

    def get(self, url, params=None, timeout=None):
        self.calls.append((url, params, timeout))
        return Response(self.payloads[url])


def test_curated_catalog_merges_lobstr_soroswap_and_stellarexpert(tmp_path):
    usdc = Keypair.random().public_key
    aqua = Keypair.random().public_key
    xrp = Keypair.random().public_key
    lobstr_url = "https://lists.example/lobstr.json"
    soroswap_url = "https://lists.example/soroswap.json"
    expert_url = "https://lists.example/stellar-expert.json"
    descriptors = [
        {"name": "Lobstr Curated List", "provider": "UltraStellar", "url": lobstr_url},
        {"name": "Soroswap List", "provider": "SoroswapFinance", "url": soroswap_url},
        {"name": "StellarExpert Top 50", "provider": "StellarExpert", "url": expert_url},
    ]
    payloads = {
        ASSET_LIST_CATALOGUE: descriptors,
        lobstr_url: {"assets": [{"code": "USDC", "issuer": usdc, "name": "USD Coin", "domain": "circle.com"}]},
        soroswap_url: {"assets": [{"code": "USDC", "issuer": usdc}, {"code": "AQUA", "issuer": aqua, "domain": "aqua.network"}]},
        expert_url: {"assets": [{"code": "USDC", "issuer": usdc}, {"code": "XRP", "issuer": xrp, "domain": "fchain.io"}]},
    }
    path = tmp_path / "assets.json"
    service = AssetCatalogService(path, session=MultiSession(payloads))

    entries = service.curated("mainnet", limit=50)

    assert [item.identity for item in entries] == [
        "XLM", f"USDC:{usdc}", f"AQUA:{aqua}", f"XRP:{xrp}"
    ]
    assert entries[1].source == "lobstr+soroswap+stellar-expert"
    assert entries[1].domain == "circle.com"
    assert service.curated_path.name == "assets.curated.json"
    cached = AssetCatalogService(path).cached_curated("mainnet")
    assert [item.identity for item in cached] == [item.identity for item in entries]

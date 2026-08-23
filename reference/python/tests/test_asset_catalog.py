import json

import requests
from stellar_sdk import Keypair

from fresnica.asset_catalog import AssetCatalogService


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

    def get(self, url, params, timeout):
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

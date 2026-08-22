from stellar_sdk import Keypair

from fresnica.datastore import MemoryDataStore
from fresnica.dex_service import DexService, asset_pair_key, resolution_value
from fresnica.models import Asset
from fresnica.wallet import Wallet


class FakeAdapter:
    def __init__(self):
        self.calls = []
        self.offers = {"_embedded": {"records": [{"id": "1", "paging_token": "1"}]}}
        self.trades = {
            "_embedded": {
                "records": [
                    {
                        "id": "2-0",
                        "paging_token": "2-0",
                        "ledger_close_time": "2026-08-22T12:00:00Z",
                    }
                ]
            }
        }
        self.aggregations = {
            "_embedded": {
                "records": [
                    {
                        "timestamp": "1787400000000",
                        "trade_count": 1,
                    }
                ]
            }
        }

    def get_orderbook(self, selling, buying):
        self.calls.append(("orderbook", selling, buying))
        return {"bids": [{"price": "1", "amount": "2"}], "asks": []}

    def get_offers(self, address, limit=20):
        self.calls.append(("offers", address, limit))
        return self.offers

    def get_trades(self, base, counter, limit=20):
        self.calls.append(("trades", base, counter, limit))
        return self.trades

    def get_trade_aggregations(
        self,
        base,
        counter,
        resolution,
        start_time=None,
        end_time=None,
        offset=None,
        limit=100,
    ):
        self.calls.append(
            (
                "aggregations",
                base,
                counter,
                resolution,
                start_time,
                end_time,
                offset,
                limit,
            )
        )
        return self.aggregations


def test_dex_service_uses_asset_direction_and_cache():
    adapter = FakeAdapter()
    store = MemoryDataStore()
    service = DexService(adapter, store, "mainnet")
    issuer = Keypair.random().public_key
    pair_asset = f"USD:{issuer}"

    orderbook = service.get_orderbook("XLM", pair_asset)
    assert orderbook["bids"][0]["price"] == "1"
    _, selling, buying = adapter.calls[-1]
    assert selling == Asset("XLM")
    assert buying == Asset("USD", issuer)

    wallet = Wallet.from_address(Keypair.random().public_key)
    assert service.get_offers(wallet, limit=5) == adapter.offers["_embedded"]["records"]
    adapter.offers = {"_embedded": {"records": []}}
    assert service.get_offers(wallet, limit=5, refresh=False)[0]["id"] == "1"

    assert service.get_trades("XLM", pair_asset, limit=7)[0]["id"] == "2-0"
    adapter.trades = {"_embedded": {"records": []}}
    assert service.get_trades("XLM", pair_asset, limit=7, refresh=False)[0]["id"] == "2-0"

    candles = service.get_trade_aggregations(
        "XLM",
        pair_asset,
        resolution="1h",
        start_time=1,
        end_time=2,
        limit=24,
    )
    assert candles[0]["trade_count"] == 1
    call = adapter.calls[-1]
    assert call[3] == 3_600_000
    assert call[4:6] == (1, 2)
    assert call[-1] == 24

    adapter.aggregations = {"_embedded": {"records": []}}
    assert service.get_trade_aggregations(
        "XLM", pair_asset, resolution="1h", refresh=False
    )[0]["timestamp"] == "1787400000000"


def test_resolution_and_pair_keys_are_explicit():
    issuer = Keypair.random().public_key
    assert resolution_value("1m") == 60_000
    assert resolution_value(3_600_000) == 3_600_000
    assert asset_pair_key(Asset("XLM"), Asset("USD", issuer)) == f"XLM>USD:{issuer}"

    try:
        resolution_value("2h")
    except ValueError as exc:
        assert "Unsupported" in str(exc)
    else:
        raise AssertionError("unsupported resolution must fail")

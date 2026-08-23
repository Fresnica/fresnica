from stellar_sdk import Keypair

from fresnica.datastore import MemoryDataStore
import fresnica.dex_service as dex_service_module
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
        self.account_trade_baseline = []
        self.account_trade_increment = []
        self.account_trade_increment_pages = {}
        self.offer_pages = {}

    def get_orderbook(self, selling, buying):
        self.calls.append(("orderbook", selling, buying))
        return {"bids": [{"price": "1", "amount": "2"}], "asks": []}

    def get_offers(self, address, limit=20, cursor=None, desc=True):
        self.calls.append(("offers", address, limit, cursor, desc))
        if self.offer_pages:
            return {"_embedded": {"records": list(self.offer_pages.get(cursor, []))}}
        return self.offers

    def get_offer(self, offer_id):
        self.calls.append(("offer", str(offer_id)))
        return self.offer

    def get_trades(self, base, counter, limit=20):
        self.calls.append(("trades", base, counter, limit))
        return self.trades

    def get_account_trades(self, address, limit=200, cursor=None, desc=True):
        self.calls.append(("account-trades", address, limit, cursor, desc))
        if cursor is None:
            records = self.account_trade_baseline
        else:
            records = self.account_trade_increment_pages.get(
                cursor, self.account_trade_increment
            )
        return {"_embedded": {"records": list(records)}}

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


def _account_trade(address, issuer, token, time, base_amount="1", counter_amount="2"):
    return {
        "id": token,
        "paging_token": token,
        "ledger_close_time": time,
        "base_asset_type": "native",
        "counter_asset_type": "credit_alphanum4",
        "counter_asset_code": "USD",
        "counter_asset_issuer": issuer,
        "base_amount": base_amount,
        "counter_amount": counter_amount,
        "price": {"n": 2, "d": 1},
        "base_is_seller": True,
        "base_account": address,
        "counter_account": Keypair.random().public_key,
        "base_offer_id": "offer-1",
    }


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
    assert adapter.calls[-1] == ("offers", wallet.address(), 200, None, True)
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


def test_open_offer_lookup_verifies_wallet_ownership():
    adapter = FakeAdapter()
    service = DexService(adapter, MemoryDataStore(), "mainnet")
    wallet = Wallet.from_address(Keypair.random().public_key)
    adapter.offer = {
        "id": "7",
        "seller": wallet.address(),
        "selling": {"asset_type": "native"},
        "buying": {
            "asset_type": "credit_alphanum4",
            "asset_code": "USD",
            "asset_issuer": Keypair.random().public_key,
        },
        "amount": "1",
        "price": "2",
        "price_r": {"n": 2, "d": 1},
    }

    assert service.get_open_offer(wallet, "7").offer_id == "7"
    adapter.offer["seller"] = Keypair.random().public_key
    try:
        service.get_open_offer(wallet, "7")
    except ValueError as exc:
        assert "does not belong" in str(exc)
    else:
        raise AssertionError("foreign offer must not be mutable")


def test_account_trade_segments_bootstrap_then_increment_from_cached_cursor():
    adapter = FakeAdapter()
    store = MemoryDataStore()
    service = DexService(adapter, store, "mainnet")
    address = Keypair.random().public_key
    wallet = Wallet.from_address(address)
    issuer = Keypair.random().public_key
    adapter.account_trade_baseline = [
        _account_trade(address, issuer, "2-0", "2026-08-23T00:02:00Z"),
        _account_trade(address, issuer, "1-0", "2026-08-23T00:01:00Z"),
    ]

    segments = service.get_account_trade_segments(wallet)
    assert len(segments) == 1
    assert segments[0].trade_count == 2
    assert segments[0].base_amount == 2
    assert adapter.calls[-1] == ("account-trades", address, 200, None, True)

    adapter.account_trade_increment = [
        _account_trade(address, issuer, "3-0", "2026-08-23T00:03:00Z"),
    ]
    segments = service.get_account_trade_segments(wallet)
    assert len(segments) == 1
    assert segments[0].trade_count == 3
    assert segments[0].base_amount == 3
    assert adapter.calls[-1] == ("account-trades", address, 200, "2-0", False)

    calls_before = len(adapter.calls)
    cached = service.get_account_trade_segments(wallet, refresh=False)
    assert cached[0].trade_count == 3
    assert len(adapter.calls) == calls_before


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


def test_account_offers_refresh_pages_complete_snapshot_and_removes_stale_cache(monkeypatch):
    monkeypatch.setattr(dex_service_module, "OFFER_PAGE_LIMIT", 2)
    adapter = FakeAdapter()
    store = MemoryDataStore()
    service = DexService(adapter, store, "mainnet")
    wallet = Wallet.from_address(Keypair.random().public_key)
    adapter.offer_pages = {
        None: [
            {"id": "4", "paging_token": "4"},
            {"id": "3", "paging_token": "3"},
        ],
        "3": [
            {"id": "2", "paging_token": "2"},
            {"id": "1", "paging_token": "1"},
        ],
        "1": [],
    }

    assert [item["id"] for item in service.get_offers(wallet, limit=10)] == [
        "4", "3", "2", "1"
    ]
    assert [call[3] for call in adapter.calls if call[0] == "offers"] == [None, "3", "1"]

    adapter.offer_pages = {None: [{"id": "5", "paging_token": "5"}]}
    assert [item["id"] for item in service.get_offers(wallet, limit=10)] == ["5"]
    assert [item["id"] for item in service.get_offers(wallet, limit=10, refresh=False)] == ["5"]


def test_account_trade_snapshot_reports_bounded_incremental_sync_until_caught_up(monkeypatch):
    monkeypatch.setattr(dex_service_module, "ACCOUNT_TRADE_PAGE_LIMIT", 2)
    monkeypatch.setattr(dex_service_module, "ACCOUNT_TRADE_MAX_INCREMENTAL_PAGES", 2)
    adapter = FakeAdapter()
    store = MemoryDataStore()
    service = DexService(adapter, store, "mainnet")
    address = Keypair.random().public_key
    wallet = Wallet.from_address(address)
    issuer = Keypair.random().public_key
    adapter.account_trade_baseline = [
        _account_trade(address, issuer, "1-0", "2026-08-23T00:01:00Z"),
    ]
    baseline = service.get_account_trade_segment_snapshot(wallet)
    assert baseline.caught_up is True

    adapter.account_trade_increment_pages = {
        "1-0": [
            _account_trade(address, issuer, "2-0", "2026-08-23T00:02:00Z"),
            _account_trade(address, issuer, "3-0", "2026-08-23T00:03:00Z"),
        ],
        "3-0": [
            _account_trade(address, issuer, "4-0", "2026-08-23T00:04:00Z"),
            _account_trade(address, issuer, "5-0", "2026-08-23T00:05:00Z"),
        ],
        "5-0": [
            _account_trade(address, issuer, "6-0", "2026-08-23T00:06:00Z"),
        ],
    }
    partial = service.get_account_trade_segment_snapshot(wallet)
    assert partial.caught_up is False
    assert partial.segments[0].trade_count == 5

    caught_up = service.get_account_trade_segment_snapshot(wallet)
    assert caught_up.caught_up is True
    assert caught_up.segments[0].trade_count == 6
    cached = service.get_account_trade_segment_snapshot(wallet, refresh=False)
    assert cached.caught_up is True


def test_account_trade_increment_stall_is_never_reported_as_caught_up(monkeypatch):
    monkeypatch.setattr(dex_service_module, "ACCOUNT_TRADE_PAGE_LIMIT", 2)
    adapter = FakeAdapter()
    store = MemoryDataStore()
    service = DexService(adapter, store, "mainnet")
    address = Keypair.random().public_key
    wallet = Wallet.from_address(address)
    issuer = Keypair.random().public_key
    adapter.account_trade_baseline = [
        _account_trade(address, issuer, "1-0", "2026-08-23T00:01:00Z"),
    ]
    service.get_account_trade_segment_snapshot(wallet)
    adapter.account_trade_increment_pages = {
        "1-0": [
            _account_trade(address, issuer, "2-0", "2026-08-23T00:02:00Z"),
            _account_trade(address, issuer, "1-0", "2026-08-23T00:01:00Z"),
        ]
    }
    assert service.get_account_trade_segment_snapshot(wallet).caught_up is False

"""SDEX market and account data built on Horizon through the Stellar SDK."""

from dataclasses import dataclass

from .models import Asset
from .offer_service import open_offer_from_horizon
from .trade_segments import account_trade_from_horizon, compress_account_trades


RESOLUTIONS = {
    "1m": 60_000,
    "5m": 300_000,
    "15m": 900_000,
    "1h": 3_600_000,
    "1d": 86_400_000,
    "1w": 604_800_000,
}
OFFER_PAGE_LIMIT = 200
ACCOUNT_TRADE_PAGE_LIMIT = 200
ACCOUNT_TRADE_MAX_INCREMENTAL_PAGES = 5


@dataclass(frozen=True)
class AccountTradeSegmentSnapshot:
    segments: list
    caught_up: bool


class DexService:
    def __init__(self, adapter, datastore, network_name: str):
        self.adapter = adapter
        self.datastore = datastore
        self.network_name = network_name
        self._account_trade_caught_up: dict[str, bool] = {}

    def get_orderbook(self, selling, buying) -> dict:
        selling_asset = _asset(selling)
        buying_asset = _asset(buying)
        return self.adapter.get_orderbook(selling_asset, buying_asset)

    def get_offers(self, wallet, limit: int = 20, refresh: bool = True) -> list[dict]:
        address = wallet.address()
        if refresh:
            self._sync_offers_snapshot(address)
        return self.datastore.get_offers(self.network_name, address, limit=limit)

    def _sync_offers_snapshot(self, address: str) -> int:
        records: list[dict] = []
        cursor: str | None = None
        while True:
            response = self.adapter.get_offers(
                address,
                limit=OFFER_PAGE_LIMIT,
                cursor=cursor,
                desc=True,
            )
            page = _records(response)
            records.extend(page)
            if len(page) < OFFER_PAGE_LIMIT:
                break
            next_cursor = _paging_token(page[-1])
            if next_cursor is None or next_cursor == cursor:
                raise ValueError("Offer pagination stalled before a complete snapshot")
            cursor = next_cursor
        self.datastore.save_offers(self.network_name, address, records)
        return len(records)

    def get_open_offers(self, wallet, limit: int = 20, refresh: bool = True):
        return [
            open_offer_from_horizon(item)
            for item in self.get_offers(wallet, limit=limit, refresh=refresh)
        ]

    def get_open_offer(self, wallet, offer_id: str):
        raw = self.adapter.get_offer(offer_id)
        if raw.get("seller") != wallet.address():
            raise ValueError(f"Offer {offer_id} does not belong to this wallet")
        return open_offer_from_horizon(raw)

    def get_account_trade_segments(
        self,
        wallet,
        limit: int = 1000,
        refresh: bool = True,
    ):
        return self.get_account_trade_segment_snapshot(
            wallet,
            limit=limit,
            refresh=refresh,
        ).segments

    def get_account_trade_segment_snapshot(
        self,
        wallet,
        limit: int = 1000,
        refresh: bool = True,
    ) -> AccountTradeSegmentSnapshot:
        """Return cached fill segments plus whether bounded sync reached Horizon head."""
        address = wallet.address()
        cache_key = account_trade_cache_key(address)
        caught_up = self._account_trade_caught_up.get(address, False)
        if refresh:
            latest = self.datastore.get_trades(
                self.network_name,
                cache_key,
                limit=1,
            )
            cursor = _paging_token(latest[0]) if latest else None
            if cursor:
                caught_up = self._sync_account_trade_increment(address, cache_key, cursor)
            else:
                response = self.adapter.get_account_trades(
                    address,
                    limit=min(ACCOUNT_TRADE_PAGE_LIMIT, max(limit, 1)),
                    desc=True,
                )
                self.datastore.save_trades(
                    self.network_name,
                    cache_key,
                    response,
                )
                caught_up = True
            self._account_trade_caught_up[address] = caught_up

        raw = self.datastore.get_trades(
            self.network_name,
            cache_key,
            limit=limit,
        )
        trades = [account_trade_from_horizon(item, address) for item in raw]
        return AccountTradeSegmentSnapshot(
            segments=compress_account_trades(trades, address),
            caught_up=caught_up,
        )

    def _sync_account_trade_increment(
        self,
        address: str,
        cache_key: str,
        cursor: str,
    ) -> bool:
        next_cursor = cursor
        for _ in range(ACCOUNT_TRADE_MAX_INCREMENTAL_PAGES):
            response = self.adapter.get_account_trades(
                address,
                limit=ACCOUNT_TRADE_PAGE_LIMIT,
                cursor=next_cursor,
                desc=False,
            )
            records = _records(response)
            if not records:
                return True
            self.datastore.save_trades(self.network_name, cache_key, records)
            last_cursor = _paging_token(records[-1])
            if last_cursor is None or last_cursor == next_cursor:
                return False
            next_cursor = last_cursor
            if len(records) < ACCOUNT_TRADE_PAGE_LIMIT:
                return True
        return False

    def get_trades(
        self,
        base,
        counter,
        limit: int = 20,
        refresh: bool = True,
    ) -> list[dict]:
        base_asset = _asset(base)
        counter_asset = _asset(counter)
        pair_key = asset_pair_key(base_asset, counter_asset)
        if refresh:
            response = self.adapter.get_trades(base_asset, counter_asset, limit=limit)
            self.datastore.save_trades(self.network_name, pair_key, response)
            return _records(response)
        return self.datastore.get_trades(self.network_name, pair_key, limit=limit)

    def get_trade_aggregations(
        self,
        base,
        counter,
        resolution="1h",
        start_time: int | None = None,
        end_time: int | None = None,
        offset: int | None = None,
        limit: int = 100,
        refresh: bool = True,
    ) -> list[dict]:
        base_asset = _asset(base)
        counter_asset = _asset(counter)
        resolution_ms = resolution_value(resolution)
        pair_key = asset_pair_key(base_asset, counter_asset)
        if refresh:
            response = self.adapter.get_trade_aggregations(
                base_asset,
                counter_asset,
                resolution=resolution_ms,
                start_time=start_time,
                end_time=end_time,
                offset=offset,
                limit=limit,
            )
            self.datastore.save_trade_aggregations(
                self.network_name,
                pair_key,
                resolution_ms,
                response,
            )
            return _records(response)
        return self.datastore.get_trade_aggregations(
            self.network_name,
            pair_key,
            resolution_ms,
            limit=limit,
        )


def resolution_value(value) -> int:
    if isinstance(value, int):
        if value in RESOLUTIONS.values():
            return value
        raise ValueError(f"Unsupported trade aggregation resolution: {value}")
    text = str(value).strip().lower()
    if text in RESOLUTIONS:
        return RESOLUTIONS[text]
    try:
        numeric = int(text)
    except ValueError as exc:
        raise ValueError(f"Unsupported trade aggregation resolution: {value}") from exc
    if numeric not in RESOLUTIONS.values():
        raise ValueError(f"Unsupported trade aggregation resolution: {value}")
    return numeric


def asset_pair_key(base: Asset, counter: Asset) -> str:
    return f"{_asset_key(base)}>{_asset_key(counter)}"


def account_trade_cache_key(address: str) -> str:
    return f"account:{address}"


def _asset(value) -> Asset:
    return value if isinstance(value, Asset) else Asset.parse(value)


def _asset_key(asset: Asset) -> str:
    if asset.is_native:
        return "XLM"
    return f"{asset.code}:{asset.issuer}"


def _paging_token(item: dict) -> str | None:
    value = item.get("paging_token", item.get("id"))
    return None if value is None else str(value)


def _records(payload) -> list[dict]:
    if isinstance(payload, list):
        return list(payload)
    if isinstance(payload, dict):
        return list(payload.get("_embedded", {}).get("records", []))
    return []

"""Read-only SDEX market data built on Horizon through the Stellar SDK."""

from .models import Asset


RESOLUTIONS = {
    "1m": 60_000,
    "5m": 300_000,
    "15m": 900_000,
    "1h": 3_600_000,
    "1d": 86_400_000,
    "1w": 604_800_000,
}


class DexService:
    def __init__(self, adapter, datastore, network_name: str):
        self.adapter = adapter
        self.datastore = datastore
        self.network_name = network_name

    def get_orderbook(self, selling, buying) -> dict:
        selling_asset = _asset(selling)
        buying_asset = _asset(buying)
        return self.adapter.get_orderbook(selling_asset, buying_asset)

    def get_offers(self, wallet, limit: int = 20, refresh: bool = True) -> list[dict]:
        address = wallet.address()
        if refresh:
            response = self.adapter.get_offers(address, limit=limit)
            self.datastore.save_offers(self.network_name, address, response)
            return _records(response)
        return self.datastore.get_offers(self.network_name, address, limit=limit)

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


def _asset(value) -> Asset:
    return value if isinstance(value, Asset) else Asset.parse(value)


def _asset_key(asset: Asset) -> str:
    if asset.is_native:
        return "XLM"
    return f"{asset.code}:{asset.issuer}"


def _records(payload) -> list[dict]:
    if isinstance(payload, list):
        return list(payload)
    if isinstance(payload, dict):
        return list(payload.get("_embedded", {}).get("records", []))
    return []

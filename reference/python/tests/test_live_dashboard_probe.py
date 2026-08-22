"""Temporary live Horizon probe. Remove after PR validation."""

from fresnica.network import MAINNET
from fresnica.stellar_adapter import StellarAdapter


def test_live_horizon_liquidity_pool_and_operation_cursor():
    adapter = StellarAdapter(MAINNET)

    pools = adapter.server.liquidity_pools().limit(1).call()
    records = pools.get("_embedded", {}).get("records", [])
    assert records

    pool_id = records[0]["id"]
    pool = adapter.get_liquidity_pool(pool_id)
    assert pool["id"] == pool_id
    assert pool.get("reserves")
    assert pool.get("total_shares") is not None

    issued_asset = next(
        reserve["asset"]
        for reserve in pool["reserves"]
        if reserve.get("asset") != "native" and ":" in reserve.get("asset", "")
    )
    issuer = issued_asset.split(":", 1)[1]

    recent = adapter.get_operations(issuer, limit=2, desc=True)
    recent_records = recent.get("_embedded", {}).get("records", [])
    assert recent_records
    cursor = recent_records[-1]["paging_token"]

    # The result may legitimately be empty; this call verifies the SDK v15
    # cursor/order builder path against real mainnet Horizon.
    older = adapter.get_operations(issuer, limit=1, cursor=cursor, desc=True)
    assert "_embedded" in older

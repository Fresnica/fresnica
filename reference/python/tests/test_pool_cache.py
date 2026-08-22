from decimal import Decimal

from stellar_sdk import Keypair

from fresnica.balance_service import BalanceService
from fresnica.datastore import SQLiteDataStore
from fresnica.errors import NetworkError
from fresnica.models import Asset, BalanceView


class PoolAdapter:
    def __init__(self, pool=None, fail=False):
        self.pool = pool
        self.fail = fail

    def get_liquidity_pool(self, pool_id):
        if self.fail:
            raise NetworkError("pool endpoint unavailable")
        return self.pool


def test_liquidity_pool_cache_survives_service_instances(tmp_path):
    issuer = Keypair.random().public_key
    pool_id = "a" * 64
    pool = {
        "id": pool_id,
        "total_shares": "50.0000000",
        "reserves": [
            {"asset": "native", "amount": "100.0000000"},
            {"asset": f"USDC:{issuer}", "amount": "200.0000000"},
        ],
    }
    store = SQLiteDataStore(tmp_path / "chain.sqlite3")
    balance = BalanceView(
        asset=Asset("LP", liquidity_pool_id=pool_id),
        balance=Decimal("5"),
    )

    first = BalanceService(PoolAdapter(pool=pool), store, "mainnet")._liquidity_position(balance)
    assert [reserve.amount for reserve in first.reserves] == [Decimal("10"), Decimal("20")]
    assert store.get_liquidity_pool("mainnet", pool_id) == pool

    cached = BalanceService(PoolAdapter(fail=True), store, "mainnet")._liquidity_position(balance)
    assert [reserve.amount for reserve in cached.reserves] == [Decimal("10"), Decimal("20")]
    assert cached.error is None

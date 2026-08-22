from decimal import Decimal

from stellar_sdk import Keypair

from fresnica.balance_service import BalanceService
from fresnica.datastore import SQLiteDataStore
from fresnica.errors import NetworkError
from fresnica.models import Asset, BalanceView
from fresnica.wallet import Wallet


class PoolAdapter:
    def __init__(self, pool=None, fail=False):
        self.pool = pool
        self.fail = fail

    def get_liquidity_pool(self, pool_id):
        if self.fail:
            raise NetworkError("pool endpoint unavailable")
        return self.pool


class NoNetworkAdapter:
    def __getattr__(self, name):
        raise AssertionError(f"cached portfolio attempted network access: {name}")


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


def test_cached_portfolio_uses_only_sqlite_data(tmp_path):
    address = Keypair.random().public_key
    issuer = Keypair.random().public_key
    pool_id = "b" * 64
    store = SQLiteDataStore(tmp_path / "chain.sqlite3")
    store.save_balances(
        "mainnet",
        address,
        [
            {
                "asset_type": "native",
                "balance": "10.0000000",
                "selling_liabilities": "1.0000000",
                "buying_liabilities": "0",
                "_fresnica_available": "7.5000000",
            },
            {
                "asset_type": "credit_alphanum4",
                "asset_code": "USDC",
                "asset_issuer": issuer,
                "balance": "5.0000000",
                "selling_liabilities": "2.0000000",
                "buying_liabilities": "0",
            },
            {
                "asset_type": "liquidity_pool_shares",
                "liquidity_pool_id": pool_id,
                "balance": "5.0000000",
                "selling_liabilities": "0",
                "buying_liabilities": "0",
            },
        ],
    )
    store.save_liquidity_pool(
        "mainnet",
        pool_id,
        {
            "id": pool_id,
            "total_shares": "50.0000000",
            "reserves": [
                {"asset": "native", "amount": "100.0000000"},
                {"asset": f"USDC:{issuer}", "amount": "200.0000000"},
            ],
        },
    )

    service = BalanceService(NoNetworkAdapter(), store, "mainnet")
    balances, positions = service.get_cached_portfolio_views(Wallet.from_address(address))

    assert [item.asset.code for item in balances] == ["XLM", "USDC"]
    assert balances[0].available == Decimal("7.5000000")
    assert balances[1].available == Decimal("3.0000000")
    assert [reserve.amount for reserve in positions[0].reserves] == [
        Decimal("10"),
        Decimal("20"),
    ]

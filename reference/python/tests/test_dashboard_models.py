from decimal import Decimal

from stellar_sdk import Keypair

from fresnica.balance_service import BalanceService
from fresnica.datastore import MemoryDataStore
from fresnica.history_service import HistoryService
from fresnica.presentation import asset_source, format_amount
from fresnica.wallet import Wallet


class PortfolioAdapter:
    def __init__(self, issuer_a, issuer_b, pool_id):
        self.issuer_a = issuer_a
        self.issuer_b = issuer_b
        self.pool_id = pool_id

    def get_account(self, address):
        return {
            "account_id": address,
            "subentry_count": 3,
            "num_sponsoring": 0,
            "num_sponsored": 0,
            "balances": [
                {
                    "asset_type": "credit_alphanum4",
                    "asset_code": "USDC",
                    "asset_issuer": self.issuer_b,
                    "balance": "0.0000000",
                    "selling_liabilities": "0.0000000",
                    "buying_liabilities": "0.0000000",
                },
                {
                    "asset_type": "liquidity_pool_shares",
                    "liquidity_pool_id": self.pool_id,
                    "balance": "10.0000000",
                },
                {
                    "asset_type": "native",
                    "balance": "50.0000000",
                    "selling_liabilities": "2.0000000",
                    "buying_liabilities": "0.0000000",
                },
                {
                    "asset_type": "credit_alphanum4",
                    "asset_code": "USDC",
                    "asset_issuer": self.issuer_a,
                    "balance": "25.0000000",
                    "selling_liabilities": "5.0000000",
                    "buying_liabilities": "0.0000000",
                },
            ],
        }

    def get_base_reserve_stroops(self):
        return 5_000_000

    def get_liquidity_pool(self, pool_id):
        assert pool_id == self.pool_id
        return {
            "id": pool_id,
            "total_shares": "100.0000000",
            "reserves": [
                {"asset": "native", "amount": "1000.0000000"},
                {"asset": f"USDC:{self.issuer_a}", "amount": "2000.0000000"},
            ],
        }


def test_portfolio_sorts_xlm_distinguishes_issuers_and_models_lp_position():
    issuer_a = Keypair.random().public_key
    issuer_b = Keypair.random().public_key
    pool_id = "a" * 64
    service = BalanceService(
        PortfolioAdapter(issuer_a, issuer_b, pool_id),
        MemoryDataStore(),
        "mainnet",
    )
    wallet = Wallet.from_address(Keypair.random().public_key)

    balances, positions = service.get_portfolio_views(wallet)

    assert balances[0].asset.is_native
    assert [item.asset.code for item in balances[1:]] == ["USDC", "USDC"]
    assert asset_source(balances[1].asset) != asset_source(balances[2].asset)
    assert format_amount(Decimal("0E-7")) == "0"
    assert positions[0].shares == Decimal("10.0000000")
    assert positions[0].reserves[0].amount == Decimal("100.00000000")
    assert positions[0].reserves[1].amount == Decimal("200.00000000")


class HistoryAdapter:
    def __init__(self, account, issuer):
        self.account = account
        self.issuer = issuer
        self.calls = []
        self.round = 0

    def get_operations(self, address, limit=20, cursor=None, desc=True):
        self.calls.append((address, limit, cursor, desc))
        if cursor is None:
            records = [
                {
                    "paging_token": "100",
                    "type": "payment",
                    "created_at": "2026-08-22T12:00:00Z",
                    "from": Keypair.random().public_key,
                    "to": self.account,
                    "asset_type": "native",
                    "amount": "1.0000000",
                },
                {
                    "paging_token": "99",
                    "type": "manage_sell_offer",
                    "created_at": "2026-08-22T11:00:00Z",
                    "source_account": self.account,
                    "selling_asset_type": "credit_alphanum4",
                    "selling_asset_code": "XRP",
                    "selling_asset_issuer": self.issuer,
                    "buying_asset_type": "native",
                    "amount": "100.0000000",
                    "price": "0.3250000",
                    "offer_id": "77",
                },
            ]
        elif desc is False:
            records = [
                {
                    "paging_token": "101",
                    "type": "liquidity_pool_deposit",
                    "created_at": "2026-08-22T13:00:00Z",
                    "source_account": self.account,
                    "reserves_deposited": [
                        {"asset": "native", "amount": "5.0000000"},
                        {"asset": f"USDC:{self.issuer}", "amount": "10.0000000"},
                    ],
                }
            ]
        else:
            records = []
        return {"_embedded": {"records": records}}


def test_history_syncs_200_locally_then_incrementally_and_humanizes_activity():
    account = Keypair.random().public_key
    issuer = Keypair.random().public_key
    adapter = HistoryAdapter(account, issuer)
    datastore = MemoryDataStore()
    service = HistoryService(adapter, datastore, "mainnet")
    wallet = Wallet.from_address(account)

    first = service.get_views(wallet, limit=20, refresh=True)
    assert adapter.calls[0][1:] == (200, None, True)
    assert first[0].summary.startswith("Received 1 XLM from")
    assert "Sell offer: 100 XRP" in first[1].summary

    second = service.get_views(wallet, limit=20, refresh=True)
    assert adapter.calls[1][1:] == (200, "100", False)
    assert second[0].summary.startswith("Added liquidity: 5 XLM + 10 USDC")
    assert len(datastore.get_operations("mainnet", account, limit=200)) == 3

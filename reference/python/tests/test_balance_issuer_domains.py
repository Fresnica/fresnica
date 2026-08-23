from stellar_sdk import Keypair

from fresnica.balance_service import BalanceService, ISSUER_DOMAIN_CACHE_KEY
from fresnica.datastore import MemoryDataStore
from fresnica.errors import NetworkError
from fresnica.models import Asset
from fresnica.presentation import asset_source, short_address


class FakeWallet:
    def __init__(self, address):
        self._address = address

    def address(self):
        return self._address


class FakeAdapter:
    def __init__(self, wallet_address, issuer, home_domain, fail_first=False):
        self.wallet_address = wallet_address
        self.issuer = issuer
        self.home_domain = home_domain
        self.fail_first = fail_first
        self.issuer_lookups = 0

    def get_base_reserve_stroops(self):
        return 5_000_000

    def get_account(self, address):
        if address == self.issuer:
            self.issuer_lookups += 1
            if self.fail_first and self.issuer_lookups == 1:
                raise NetworkError("temporary issuer lookup failure")
            return {
                "account_id": self.issuer,
                "home_domain": self.home_domain,
                "balances": [],
            }
        assert address == self.wallet_address
        return {
            "account_id": self.wallet_address,
            "subentry_count": 2,
            "num_sponsoring": 0,
            "num_sponsored": 0,
            "balances": [
                {
                    "asset_type": "native",
                    "balance": "10.0000000",
                    "selling_liabilities": "0.0000000",
                    "buying_liabilities": "0.0000000",
                },
                {
                    "asset_type": "credit_alphanum4",
                    "asset_code": "USD",
                    "asset_issuer": self.issuer,
                    "balance": "5.0000000",
                    "selling_liabilities": "0.0000000",
                    "buying_liabilities": "0.0000000",
                },
                {
                    "asset_type": "credit_alphanum4",
                    "asset_code": "EUR",
                    "asset_issuer": self.issuer,
                    "balance": "2.0000000",
                    "selling_liabilities": "0.0000000",
                    "buying_liabilities": "0.0000000",
                },
            ],
        }


def _service(home_domain="issuer.example", fail_first=False):
    wallet_address = Keypair.random().public_key
    issuer = Keypair.random().public_key
    adapter = FakeAdapter(wallet_address, issuer, home_domain, fail_first=fail_first)
    service = BalanceService(adapter, MemoryDataStore(), "testnet")
    return service, adapter, FakeWallet(wallet_address), issuer


def test_issuer_domain_is_fetched_once_per_issuer_and_reused_from_balance_cache():
    service, adapter, wallet, _ = _service()

    balances, _ = service.get_portfolio_views(wallet)
    issued = [item for item in balances if not item.asset.is_native]

    assert adapter.issuer_lookups == 1
    assert {item.raw[ISSUER_DOMAIN_CACHE_KEY] for item in issued} == {"issuer.example"}

    service.get_portfolio_views(wallet)
    assert adapter.issuer_lookups == 1


def test_successful_missing_home_domain_is_cached_and_presentation_falls_back_to_address():
    service, adapter, wallet, issuer = _service(home_domain="")

    balances, _ = service.get_portfolio_views(wallet)
    issued = [item for item in balances if not item.asset.is_native]

    assert adapter.issuer_lookups == 1
    assert all(item.raw[ISSUER_DOMAIN_CACHE_KEY] == "" for item in issued)
    assert asset_source(Asset("USD", issuer), "") == short_address(issuer)

    service.get_portfolio_views(wallet)
    assert adapter.issuer_lookups == 1


def test_transient_issuer_lookup_failure_is_retried_on_next_refresh():
    service, adapter, wallet, _ = _service(fail_first=True)

    first, _ = service.get_portfolio_views(wallet)
    first_issued = [item for item in first if not item.asset.is_native]
    assert adapter.issuer_lookups == 1
    assert all(ISSUER_DOMAIN_CACHE_KEY not in item.raw for item in first_issued)

    second, _ = service.get_portfolio_views(wallet)
    second_issued = [item for item in second if not item.asset.is_native]
    assert adapter.issuer_lookups == 2
    assert all(item.raw[ISSUER_DOMAIN_CACHE_KEY] == "issuer.example" for item in second_issued)

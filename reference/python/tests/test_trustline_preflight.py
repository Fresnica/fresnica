from decimal import Decimal
from types import SimpleNamespace

import pytest

from fresnica.errors import TransactionError
from fresnica.models import Asset
from fresnica.trustline_service import TrustlineService


SOURCE = "GSOURCE"
ISSUER = "GISSUER"


class Wallet:
    def address(self):
        return SOURCE

    def can_sign(self):
        return True


class Adapter:
    def __init__(self):
        self.exists = {ISSUER: True}
        self.accounts = {
            ISSUER: {
                "account_id": ISSUER,
                "flags": {
                    "auth_required": False,
                    "auth_clawback_enabled": False,
                },
            }
        }
        self.pools = {}

    def fetch_base_fee(self):
        return 100

    def get_base_reserve_stroops(self):
        return 5_000_000

    def account_exists(self, address):
        return self.exists.get(address, False)

    def get_account(self, address):
        return self.accounts[address]

    def get_liquidity_pool(self, pool_id):
        return self.pools[pool_id]


class BalanceService:
    def __init__(self, account, adapter):
        self.account = account
        self.adapter = adapter

    def get_account(self, wallet, refresh=True):
        return self.account


class Builder:
    def __init__(self):
        self.calls = []

    def build_trustline(self, **kwargs):
        self.calls.append(kwargs)
        return SimpleNamespace(review=kwargs, envelope="envelope")


class TransactionService:
    pass


def native():
    return {
        "asset_type": "native",
        "balance": "3",
        "selling_liabilities": "0",
        "buying_liabilities": "0",
    }


def line(*, authorization="full", clawback=False):
    full = authorization == "full"
    maintain = authorization in {"full", "maintain_liabilities"}
    return {
        "asset_type": "credit_alphanum4",
        "asset_code": "USD",
        "asset_issuer": ISSUER,
        "balance": "0",
        "buying_liabilities": "0",
        "selling_liabilities": "0",
        "limit": "1000",
        "is_authorized": full,
        "is_authorized_to_maintain_liabilities": maintain,
        "is_clawback_enabled": clawback,
    }


def account(*balances):
    return {
        "account_id": SOURCE,
        "balances": [native(), *balances],
        "subentry_count": len(balances),
        "num_sponsoring": 0,
        "num_sponsored": 0,
    }


def service(source_account, adapter=None):
    adapter = adapter or Adapter()
    builder = Builder()
    return (
        TrustlineService(
            BalanceService(source_account, adapter),
            builder,
            TransactionService(),
        ),
        builder,
        adapter,
    )


def test_add_requires_issuer_and_surfaces_initial_authorization_and_clawback():
    svc, builder, adapter = service(account())
    asset = Asset("USD", ISSUER)
    adapter.accounts[ISSUER]["flags"] = {
        "auth_required": True,
        "auth_clawback_enabled": True,
    }

    svc.prepare_add("main", Wallet(), asset)
    assert builder.calls[-1]["authorization"] == "unauthorized"
    assert builder.calls[-1]["clawback_enabled"] is True

    adapter.exists[ISSUER] = False
    with pytest.raises(TransactionError, match="issuer account does not exist"):
        svc.prepare_add("main", Wallet(), asset)


def test_limit_requires_live_issuer_and_preserves_existing_state():
    svc, builder, adapter = service(account(line(authorization="maintain_liabilities", clawback=True)))
    asset = Asset("USD", ISSUER)

    svc.prepare_limit("main", Wallet(), asset, Decimal("1000"))
    assert builder.calls[-1]["authorization"] == "maintain_liabilities"
    assert builder.calls[-1]["clawback_enabled"] is True

    adapter.exists[ISSUER] = False
    with pytest.raises(TransactionError, match="issuer account does not exist"):
        svc.prepare_limit("main", Wallet(), asset, Decimal("1000"))


def test_remove_allows_orphaned_issuer_but_rejects_pool_reference():
    pool_share = {
        "asset_type": "liquidity_pool_shares",
        "liquidity_pool_id": "pool-1",
        "balance": "0",
    }
    svc, builder, adapter = service(account(line(), pool_share))
    asset = Asset("USD", ISSUER)
    adapter.exists[ISSUER] = False
    adapter.pools["pool-1"] = {
        "reserves": [
            {"asset": "native", "amount": "1"},
            {"asset": f"USD:{ISSUER}", "amount": "1"},
        ]
    }

    with pytest.raises(TransactionError, match="liquidity pool pool-1 uses"):
        svc.prepare_remove("main", Wallet(), asset)

    adapter.pools["pool-1"] = {
        "reserves": [
            {"asset": "native", "amount": "1"},
            {"asset": "EUR:GANOTHER", "amount": "1"},
        ]
    }
    svc.prepare_remove("main", Wallet(), asset)
    assert builder.calls[-1]["action"] == "remove"

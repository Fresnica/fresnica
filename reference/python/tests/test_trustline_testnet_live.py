"""Opt-in standalone trustline lifecycle probe against Stellar Testnet.

Run with:

    FRESNICA_LIVE_TESTNET=1 uv run pytest -q -s tests/test_trustline_testnet_live.py

The test creates disposable Testnet accounts and never prints or persists their
secret keys.
"""

import os
import time

import pytest
from stellar_sdk import Keypair

from fresnica.balance_service import BalanceService
from fresnica.datastore import MemoryDataStore
from fresnica.friendbot import FriendbotService
from fresnica.models import Asset
from fresnica.network import TESTNET
from fresnica.stellar_adapter import StellarAdapter
from fresnica.submit_service import SubmitService
from fresnica.transaction_builder_service import TransactionBuilderService
from fresnica.transaction_service import TransactionService
from fresnica.trustline_service import TrustlineService
from fresnica.wallet import Wallet


pytestmark = pytest.mark.skipif(
    os.environ.get("FRESNICA_LIVE_TESTNET") != "1",
    reason="set FRESNICA_LIVE_TESTNET=1 to run live Testnet probes",
)


def _fund(friendbot, wallet):
    response = friendbot.fund(wallet.address())
    assert response.get("hash")


def _trustline(account, asset):
    for raw in account.get("balances", []):
        if (
            raw.get("asset_code") == asset.code
            and raw.get("asset_issuer") == asset.issuer
        ):
            return raw
    return None


def _wait_for_trustline(adapter, wallet, asset, *, present, limit=None):
    for _ in range(12):
        line = _trustline(adapter.get_account(wallet.address()), asset)
        if present and line is not None:
            if limit is None or line.get("limit") == limit:
                return line
        if not present and line is None:
            return None
        time.sleep(0.5)
    state = adapter.get_account(wallet.address())
    line = _trustline(state, asset)
    if present:
        assert line is not None
        if limit is not None:
            assert line.get("limit") == limit
        return line
    assert line is None
    return None


def _submit(service, wallet, prepared):
    service.sign(wallet, prepared)
    result = service.submit(prepared)
    assert result.successful
    assert result.hash
    return result


def test_standalone_trustline_add_limit_remove_roundtrip():
    adapter = StellarAdapter(TESTNET)
    friendbot = FriendbotService()
    holder = Wallet.from_secret(Keypair.random().secret)
    issuer = Wallet.from_secret(Keypair.random().secret)

    _fund(friendbot, holder)
    _fund(friendbot, issuer)

    balance = BalanceService(adapter, MemoryDataStore(), TESTNET.name)
    builder = TransactionBuilderService(adapter)
    transaction = TransactionService(SubmitService(adapter))
    service = TrustlineService(balance, builder, transaction)
    asset = Asset("TST", issuer.address())

    added = service.prepare_add("holder", holder, asset, limit="100")
    assert added.review.action == "add"
    assert added.review.limit == "100"
    _submit(service, holder, added)
    _wait_for_trustline(
        adapter,
        holder,
        asset,
        present=True,
        limit="100.0000000",
    )

    updated = service.prepare_limit("holder", holder, asset, "250")
    assert updated.review.action == "limit"
    assert updated.review.limit == "250"
    _submit(service, holder, updated)
    _wait_for_trustline(
        adapter,
        holder,
        asset,
        present=True,
        limit="250.0000000",
    )

    removed = service.prepare_remove("holder", holder, asset)
    assert removed.review.action == "remove"
    assert removed.review.limit is None
    _submit(service, holder, removed)
    _wait_for_trustline(adapter, holder, asset, present=False)

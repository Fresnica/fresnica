"""Opt-in transfer safety probes against Stellar Testnet.

Run with:

    FRESNICA_LIVE_TESTNET=1 uv run pytest -q -s tests/test_transfer_testnet_live.py

The test creates disposable Testnet accounts and never prints or persists their
secret keys.
"""

import os

import pytest
from stellar_sdk import Keypair, TransactionBuilder

from fresnica.errors import MemoRequiredError
from fresnica.friendbot import FriendbotService
from fresnica.models import Asset
from fresnica.network import TESTNET
from fresnica.stellar_adapter import StellarAdapter
from fresnica.wallet import Wallet


pytestmark = pytest.mark.skipif(
    os.environ.get("FRESNICA_LIVE_TESTNET") != "1",
    reason="set FRESNICA_LIVE_TESTNET=1 to run live Testnet probes",
)


def _fund(friendbot, wallet):
    response = friendbot.fund(wallet.address())
    assert response.get("hash")


def _require_memo(adapter, wallet):
    fee = adapter.fetch_base_fee()
    account = adapter.server.load_account(wallet.address())
    envelope = (
        TransactionBuilder(
            source_account=account,
            network_passphrase=TESTNET.passphrase,
            base_fee=fee,
        )
        .append_manage_data_op(
            data_name="config.memo_required",
            data_value="1",
        )
        .set_timeout(30)
        .build()
    )
    wallet.sign(envelope)
    response = adapter.submit_transaction(envelope)
    assert response.get("successful", True)


def _payment(adapter, source, destination, memo=None):
    envelope = adapter.build_payment(
        source=source.address(),
        destination=destination,
        asset=Asset("XLM"),
        amount="1",
        base_fee=adapter.fetch_base_fee(),
        memo=memo,
    )
    source.sign(envelope)
    return envelope


def test_sep29_destination_blocks_missing_memo_and_accepts_present_memo():
    adapter = StellarAdapter(TESTNET)
    friendbot = FriendbotService()
    source = Wallet.from_secret(Keypair.random().secret)
    destination = Wallet.from_secret(Keypair.random().secret)

    _fund(friendbot, source)
    _fund(friendbot, destination)
    _require_memo(adapter, destination)

    with pytest.raises(MemoRequiredError) as captured:
        adapter.submit_transaction(_payment(adapter, source, destination.address()))

    assert captured.value.account_id == destination.address()

    response = adapter.submit_transaction(
        _payment(
            adapter,
            source,
            destination.address(),
            memo="fresnica-sep29-probe",
        )
    )
    assert response.get("successful", True)
    assert response.get("hash")

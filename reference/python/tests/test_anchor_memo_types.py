import base64
from decimal import Decimal

from stellar_sdk import Account, Keypair

from fresnica.models import Asset
from fresnica.network import get_network
from fresnica.stellar_adapter import StellarAdapter


class Server:
    def __init__(self, account, source):
        self.account = account
        self.source = source

    def load_account(self, source):
        assert source == self.source
        return self.account


def test_anchor_hash_memo_builds_real_hash_memo():
    keypair = Keypair.random()
    adapter = StellarAdapter(get_network("testnet"))
    adapter.server = Server(Account(keypair.public_key, 1), keypair.public_key)
    memo_bytes = bytes(range(32))
    memo = base64.b64encode(memo_bytes).decode("ascii")

    envelope = adapter.build_payment(
        source=keypair.public_key,
        destination=Keypair.random().public_key,
        asset=Asset("XLM"),
        amount="1",
        base_fee=100,
        memo=memo,
        memo_type="hash",
    )

    assert envelope.transaction.memo.memo_hash == memo_bytes

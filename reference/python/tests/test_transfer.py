from stellar_sdk import Keypair

from fresnica.balance_service import BalanceService
from fresnica.datastore import MemoryDataStore
from fresnica.network import MAINNET
from fresnica.submit_service import SubmitService
from fresnica.transaction_builder_service import TransactionBuilderService
from fresnica.transaction_service import TransactionService
from fresnica.transfer_service import TransferService
from fresnica.wallet import Wallet


class FakeEnvelope:
    def __init__(self):
        self.signatures = []

    def sign(self, keypair):
        self.signatures.append(keypair.public_key)


class FakeAdapter:
    network = MAINNET

    def __init__(self):
        self.submitted = None

    def get_account(self, address):
        return {
            "account_id": address,
            "subentry_count": 0,
            "num_sponsoring": 0,
            "num_sponsored": 0,
            "balances": [
                {
                    "asset_type": "native",
                    "balance": "10",
                    "selling_liabilities": "0",
                    "buying_liabilities": "0",
                }
            ],
        }

    def fetch_base_fee(self):
        return 100

    def get_base_reserve_stroops(self):
        return 5_000_000

    def build_payment(self, **kwargs):
        self.build_kwargs = kwargs
        return FakeEnvelope()

    def submit_transaction(self, transaction):
        self.submitted = transaction
        return {"hash": "abc123", "ledger": 42, "successful": True}


def test_prepare_sign_submit_payment_flow():
    adapter = FakeAdapter()
    balance = BalanceService(adapter, MemoryDataStore(), "mainnet")
    builder = TransactionBuilderService(adapter)
    transaction = TransactionService(SubmitService(adapter))
    transfer = TransferService(balance, builder, transaction)
    wallet = Wallet.from_secret(Keypair.random().secret)

    prepared = transfer.prepare(
        "main",
        wallet,
        destination=Keypair.random().public_key,
        asset="XLM",
        amount="1",
    )
    assert prepared.review.wallet_name == "main"
    assert prepared.review.amount == "1"

    transfer.sign(wallet, prepared)
    assert prepared.envelope.signatures == [wallet.address()]

    result = transfer.submit(prepared)
    assert result.hash == "abc123"
    assert result.ledger == 42

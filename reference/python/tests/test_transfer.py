import pytest
from stellar_sdk import Keypair

from fresnica.balance_service import BalanceService
from fresnica.datastore import MemoryDataStore
from fresnica.errors import InvalidAmountError, TransactionError
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

    def __init__(self, destination_exists=True):
        self.submitted = None
        self.destination_exists = destination_exists

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

    def account_exists(self, address):
        return self.destination_exists

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


def _services(adapter):
    balance = BalanceService(adapter, MemoryDataStore(), "mainnet")
    builder = TransactionBuilderService(adapter)
    transaction = TransactionService(SubmitService(adapter))
    return TransferService(balance, builder, transaction)


def test_prepare_sign_submit_payment_flow():
    adapter = FakeAdapter()
    transfer = _services(adapter)
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
    assert prepared.review.operation == "payment"
    assert adapter.build_kwargs["create_destination"] is False

    transfer.sign(wallet, prepared)
    assert prepared.envelope.signatures == [wallet.address()]

    result = transfer.submit(prepared)
    assert result.hash == "abc123"
    assert result.ledger == 42


def test_missing_destination_uses_create_account_for_xlm():
    adapter = FakeAdapter(destination_exists=False)
    transfer = _services(adapter)
    wallet = Wallet.from_secret(Keypair.random().secret)

    prepared = transfer.prepare(
        "main",
        wallet,
        destination=Keypair.random().public_key,
        asset="XLM",
        amount="1",
    )

    assert prepared.review.operation == "create_account"
    assert adapter.build_kwargs["create_destination"] is True


def test_missing_destination_rejects_issued_asset():
    adapter = FakeAdapter(destination_exists=False)
    transfer = _services(adapter)
    wallet = Wallet.from_secret(Keypair.random().secret)

    with pytest.raises(TransactionError, match="Only XLM can create"):
        transfer.prepare(
            "main",
            wallet,
            destination=Keypair.random().public_key,
            asset=f"USDC:{Keypair.random().public_key}",
            amount="1",
        )


def test_create_account_requires_current_minimum_balance():
    adapter = FakeAdapter(destination_exists=False)
    transfer = _services(adapter)
    wallet = Wallet.from_secret(Keypair.random().secret)

    with pytest.raises(InvalidAmountError, match="requires at least 1 XLM"):
        transfer.prepare(
            "main",
            wallet,
            destination=Keypair.random().public_key,
            asset="XLM",
            amount="0.5",
        )

from stellar_sdk import Keypair

from fresnica.balance_service import BalanceService
from fresnica.datastore import MemoryDataStore
from fresnica.friendbot import FriendbotService
from fresnica.network import TESTNET
from fresnica.stellar_adapter import StellarAdapter
from fresnica.submit_service import SubmitService
from fresnica.transaction_builder_service import TransactionBuilderService
from fresnica.transaction_service import TransactionService
from fresnica.transfer_service import TransferService
from fresnica.wallet import Wallet


def test_live_send_xlm_creates_missing_testnet_account():
    source = Keypair.random()
    destination = Keypair.random()
    FriendbotService().fund(source.public_key)

    adapter = StellarAdapter(TESTNET)
    balance = BalanceService(adapter, MemoryDataStore(), "testnet")
    builder = TransactionBuilderService(adapter)
    transaction = TransactionService(SubmitService(adapter))
    transfer = TransferService(balance, builder, transaction)
    wallet = Wallet.from_secret(source.secret)

    prepared = transfer.prepare(
        "live-source",
        wallet,
        destination=destination.public_key,
        asset="XLM",
        amount="1",
    )
    assert prepared.review.operation == "create_account"

    transfer.sign(wallet, prepared)
    result = transfer.submit(prepared)

    assert result.successful
    assert result.hash
    assert adapter.account_exists(destination.public_key)

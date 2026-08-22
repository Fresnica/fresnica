from stellar_sdk import Keypair

from fresnica.datastore import MemoryDataStore
from fresnica.runtime import Runtime
from fresnica.storage import MemoryWalletStorage


def test_live_testnet_friendbot_balance_and_payment(tmp_path):
    runtime = Runtime(
        network="testnet",
        home=tmp_path,
        wallet_storage=MemoryWalletStorage(),
        datastore=MemoryDataStore(),
    )
    services = runtime.services_for()

    source = Keypair.random()
    destination = Keypair.random()

    runtime.wallet_manager.import_secret(
        "ci-source",
        source.secret,
        "temporary-password",
        network="testnet",
    )

    services.testnet_service.fund(source.public_key)
    services.testnet_service.fund(destination.public_key)

    session = runtime.wallet_manager.unlock("ci-source", "temporary-password")
    balances = services.balance_service.get_views(session.wallet)
    assert any(item.asset.is_native and item.balance > 0 for item in balances)

    prepared = services.transfer_service.prepare(
        wallet_name="ci-source",
        wallet=session.wallet,
        destination=destination.public_key,
        asset="XLM",
        amount="1",
    )
    services.transfer_service.sign(session.wallet, prepared)
    result = services.transfer_service.submit(prepared)

    assert result.successful is True
    assert result.hash
    assert result.ledger is not None

from fresnica.datastore import MemoryDataStore
from fresnica.runtime import Runtime
from fresnica.storage import MemoryWalletStorage


def test_runtime_composes_and_caches_network_services(tmp_path):
    runtime = Runtime(
        home=tmp_path,
        wallet_storage=MemoryWalletStorage(),
        datastore=MemoryDataStore(),
    )

    mainnet = runtime.services_for("mainnet")
    assert mainnet is runtime.services_for("mainnet")
    assert mainnet is not runtime.services_for("testnet")
    assert runtime.wallet_manager.storage is runtime.wallet_storage
    assert runtime.contact_store.path == tmp_path / "contacts.json"
    assert mainnet.pending_transaction_service.store is runtime.pending_transaction_store


def test_testnet_runtime_uses_one_shared_service_graph(tmp_path):
    runtime = Runtime(
        network="testnet",
        home=tmp_path,
        wallet_storage=MemoryWalletStorage(),
        datastore=MemoryDataStore(),
    )

    services = runtime.services_for()

    assert services is runtime.services_for("testnet")
    assert services.testnet_service is not None
    assert services.transfer_service.balance_service is services.balance_service
    assert services.transfer_service.transaction_builder is services.transaction_builder
    assert services.transfer_service.transaction_service is services.transaction_service
    assert services.offer_service.transaction_builder is services.transaction_builder
    assert services.offer_service.transaction_service is services.transaction_service
    assert services.transaction_service.submit_service is services.submit_service
    assert services.transaction_service.pending_service is services.pending_transaction_service

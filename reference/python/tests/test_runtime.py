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

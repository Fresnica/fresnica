import json

from stellar_sdk import Keypair

from fresnica.manager import WalletManager, WalletState
from fresnica.storage import MemoryWalletStorage


def test_wallet_state_and_capabilities_follow_session():
    manager = WalletManager(MemoryWalletStorage())
    keypair = Keypair.random()
    manager.import_secret("signing", keypair.secret, "pw", network="testnet")
    manager.add_watch("observer", Keypair.random().public_key, network="mainnet")

    assert manager.state("signing") is WalletState.LOCKED
    capabilities = manager.capabilities("signing")
    assert capabilities.can_send
    assert capabilities.can_unlock
    assert capabilities.can_fund_testnet

    manager.unlock("signing", "pw")
    assert manager.state("signing") is WalletState.UNLOCKED
    assert manager.capabilities("signing").can_lock

    observer = manager.capabilities("observer")
    assert observer.state is WalletState.WATCH_ONLY
    assert not observer.can_send
    assert not observer.can_unlock
    assert not observer.can_lock
    assert not observer.can_fund_testnet

    manager.set_default("observer")
    assert manager.current() is None
    assert manager.state("signing") is WalletState.LOCKED


def test_create_mnemonic_uses_wallet_lifecycle_and_keeps_secret_encrypted():
    manager = WalletManager(MemoryWalletStorage())
    record, mnemonic = manager.create_mnemonic(
        "created",
        "pw",
        language="english",
        strength=128,
        network="testnet",
    )

    assert record.wallet_type == "mnemonic"
    assert record.network == "testnet"
    assert mnemonic not in json.dumps(record.to_dict())
    assert manager.state("created") is WalletState.LOCKED
    assert manager.unlock("created", "pw").wallet.address() == record.address


def test_deleting_default_wallet_selects_a_remaining_wallet():
    manager = WalletManager(MemoryWalletStorage())
    first = Keypair.random()
    second = Keypair.random()
    manager.import_secret("alpha", first.secret, "pw", make_default=True)
    manager.add_watch("beta", second.public_key, make_default=False)

    manager.delete("alpha")

    assert manager.get_record().name == "beta"
    assert manager.storage.get_default() == "beta"

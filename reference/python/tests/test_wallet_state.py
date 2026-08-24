import json

import pytest
from stellar_sdk import Keypair

from fresnica.errors import InvalidPasswordError, WalletLockedError
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
    assert record.signer_kind == "protected-software"
    assert record.signer_public_key == record.address
    assert record.network == "testnet"
    assert mnemonic not in json.dumps(record.to_dict())
    assert manager.state("created") is WalletState.LOCKED
    assert manager.unlock("created", "pw").wallet.address() == record.address


def test_watch_only_can_attach_matching_secret_and_detach_without_changing_account():
    manager = WalletManager(MemoryWalletStorage())
    keypair = Keypair.random()
    watched = manager.add_watch("observer", keypair.public_key, network="testnet")

    upgraded = manager.upgrade_watch_with_secret("observer", keypair.secret, "pw")
    assert upgraded.address == watched.address
    assert upgraded.signer_public_key == keypair.public_key
    assert upgraded.signer_kind == "protected-software"
    assert manager.state("observer") is WalletState.LOCKED

    downgraded = manager.downgrade_to_watch("observer")
    assert downgraded.address == watched.address
    assert downgraded.signer_public_key is None
    assert downgraded.secret is None
    assert manager.state("observer") is WalletState.WATCH_ONLY


def test_watch_only_rejects_non_matching_secret():
    manager = WalletManager(MemoryWalletStorage())
    manager.add_watch("observer", Keypair.random().public_key)

    with pytest.raises(WalletLockedError, match="Signer identity does not match"):
        manager.upgrade_watch_with_secret("observer", Keypair.random().secret, "pw")

    assert manager.get_record("observer").watch_only


def test_reprotect_changes_password_without_changing_signer_identity():
    manager = WalletManager(MemoryWalletStorage())
    keypair = Keypair.random()
    original = manager.import_secret("signing", keypair.secret, "old")

    changed = manager.reprotect_signer("signing", "old", "new")
    assert changed.address == original.address
    assert changed.signer_public_key == original.signer_public_key
    assert changed.secret != original.secret

    with pytest.raises(InvalidPasswordError):
        manager.unlock("signing", "old")
    assert manager.unlock("signing", "new").wallet.signer_public_key() == keypair.public_key


def test_deleting_default_wallet_selects_a_remaining_wallet():
    manager = WalletManager(MemoryWalletStorage())
    first = Keypair.random()
    second = Keypair.random()
    manager.import_secret("alpha", first.secret, "pw", make_default=True)
    manager.add_watch("beta", second.public_key, make_default=False)

    manager.delete("alpha")

    assert manager.get_record().name == "beta"
    assert manager.storage.get_default() == "beta"

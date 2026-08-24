import json

import pytest
from stellar_sdk import Keypair

from fresnica.errors import InvalidPasswordError, WatchOnlyError
from fresnica.hdwallet import generate_mnemonic_phrase
from fresnica.manager import WalletManager
from fresnica.storage import MemoryWalletStorage


def test_watch_wallet_is_viewable_but_not_unlockable():
    manager = WalletManager(MemoryWalletStorage())
    public_key = Keypair.random().public_key
    manager.add_watch("watch", public_key)

    assert manager.view().wallet.address() == public_key
    assert not manager.has_app_passcode()
    with pytest.raises(WatchOnlyError):
        manager.unlock("watch", "anything")


def test_secret_wallet_is_encrypted_and_unlocks():
    manager = WalletManager(MemoryWalletStorage())
    keypair = Keypair.random()
    record = manager.import_secret("main", keypair.secret, "password")

    assert keypair.secret not in json.dumps(record.to_dict())
    assert manager.has_app_passcode()
    session = manager.unlock("main", "password")
    assert session.wallet.address() == keypair.public_key
    assert session.wallet.can_sign()

    manager.lock()
    with pytest.raises(InvalidPasswordError):
        manager.unlock("main", "wrong")


def test_new_signing_wallet_must_use_existing_fresnica_passcode():
    manager = WalletManager(MemoryWalletStorage())
    first = Keypair.random()
    second = Keypair.random()
    manager.import_secret("first", first.secret, "shared-passcode")

    with pytest.raises(InvalidPasswordError, match="Invalid Fresnica passcode"):
        manager.import_secret("second", second.secret, "different-passcode")

    record = manager.import_secret("second", second.secret, "shared-passcode")
    assert manager.unlock(record.name, "shared-passcode").wallet.address() == second.public_key


def test_chinese_mnemonic_round_trip():
    manager = WalletManager(MemoryWalletStorage())
    mnemonic = generate_mnemonic_phrase(language="chinese_simplified", strength=128)
    record = manager.import_mnemonic(
        "cn",
        mnemonic,
        "password",
        language="chinese_simplified",
    )

    session = manager.unlock("cn", "password")
    assert session.wallet.address() == record.address

import pytest
from stellar_sdk import Keypair

from fresnica.errors import InvalidPasswordError, InvalidUnlockKeyError
from fresnica.manager import WalletManager
from fresnica.protection import (
    PasswordProtectionProvider,
    ProtectionCredential,
    ProtectionRegistry,
)
from fresnica.secret_store import WalletUnlockKey, encrypt_secret
from fresnica.storage import MemoryWalletStorage, WalletRecord


def test_password_provider_is_a_registry_implementation():
    registry = ProtectionRegistry()
    credential = ProtectionCredential.password("correct")
    envelope = registry.protect({"kind": "secret", "secret": "S..."}, credential)

    assert registry.kind_for(envelope) == "password"
    assert "S..." not in str(envelope)
    assert registry.unprotect(envelope, credential)["secret"] == "S..."
    with pytest.raises(InvalidPasswordError):
        registry.unprotect(envelope, ProtectionCredential.password("wrong"))


def test_same_password_envelope_opens_with_unlock_key():
    registry = ProtectionRegistry()
    credential = ProtectionCredential.password("correct")
    envelope = registry.protect({"kind": "secret", "secret": "S..."}, credential)
    unlock_key = registry.derive_unlock_key(envelope, "correct")

    assert repr(unlock_key) == "WalletUnlockKey(<redacted>)"
    assert registry.unprotect_with_unlock_key(envelope, unlock_key)["secret"] == "S..."
    with pytest.raises(InvalidUnlockKeyError):
        registry.unprotect_with_unlock_key(envelope, WalletUnlockKey(bytes(32)))


def test_legacy_password_envelope_is_readable_and_migratable():
    provider = PasswordProtectionProvider()
    credential = ProtectionCredential.password("password")
    legacy = provider.protect({"kind": "secret", "secret": "S..."}, credential)
    registry = ProtectionRegistry()

    assert registry.kind_for(legacy) == "password"
    assert registry.unprotect(legacy, credential)["secret"] == "S..."
    migrated = registry.migrate_legacy_password(legacy)
    assert registry.kind_for(migrated) == "password"
    assert registry.unprotect(migrated, credential)["secret"] == "S..."


def test_wallet_manager_derives_verified_unlock_key_and_unlocks_with_it():
    manager = WalletManager(MemoryWalletStorage())
    keypair = Keypair.random()
    manager.import_secret("wallet", keypair.secret, "correct")

    unlock_key = manager.derive_verified_unlock_key("wallet", "correct")
    session = manager.unlock_with_key("wallet", unlock_key)

    assert session.wallet.address() == keypair.public_key


def test_wallet_manager_wrong_passcode_cannot_enroll_unlock_key():
    manager = WalletManager(MemoryWalletStorage())
    keypair = Keypair.random()
    manager.import_secret("wallet", keypair.secret, "correct")

    with pytest.raises(InvalidPasswordError):
        manager.derive_verified_unlock_key("wallet", "wrong")


def test_wallet_manager_wrong_unlock_key_cannot_unlock():
    manager = WalletManager(MemoryWalletStorage())
    keypair = Keypair.random()
    manager.import_secret("wallet", keypair.secret, "correct")

    with pytest.raises(InvalidUnlockKeyError):
        manager.unlock_with_key("wallet", WalletUnlockKey(bytes(32)))


def test_wallet_manager_can_explicitly_upgrade_legacy_password_metadata():
    storage = MemoryWalletStorage()
    keypair = Keypair.random()
    legacy = encrypt_secret({"kind": "secret", "secret": keypair.secret}, "password")
    storage.save(
        WalletRecord(
            name="legacy",
            address=keypair.public_key,
            wallet_type="secret",
            secret=legacy,
        )
    )
    manager = WalletManager(storage)

    assert manager.protection_kind("legacy") == "password"
    manager.unlock("legacy", "password")
    upgraded = manager.upgrade_legacy_protection(
        "legacy",
        ProtectionCredential.password("password"),
    )

    assert upgraded.secret["format"] == "fresnica-protected-secret"
    assert upgraded.secret["payload"] == legacy
    manager.lock()
    assert manager.unlock("legacy", "password").wallet.address() == keypair.public_key

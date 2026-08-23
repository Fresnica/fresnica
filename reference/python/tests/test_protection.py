import pytest
from stellar_sdk import Keypair

from fresnica.errors import InvalidPasswordError, ProtectionError
from fresnica.manager import WalletManager
from fresnica.protection import (
    PasswordProtectionProvider,
    ProtectionCredential,
    ProtectionRegistry,
    SystemKeyStore,
    SystemProtectionProvider,
)
from fresnica.secret_store import encrypt_secret
from fresnica.storage import MemoryWalletStorage, WalletRecord


class MemorySystemKeyStore(SystemKeyStore):
    def __init__(self):
        self.keys = {}
        self.next_id = 0

    def store_key(self, key: bytes) -> str:
        reference = f"key-{self.next_id}"
        self.next_id += 1
        self.keys[reference] = key
        return reference

    def load_key(self, reference: str) -> bytes:
        return self.keys[reference]


def test_password_provider_is_a_registry_implementation():
    registry = ProtectionRegistry()
    credential = ProtectionCredential.password("correct")
    envelope = registry.protect({"kind": "secret", "secret": "S..."}, credential)

    assert registry.kind_for(envelope) == "password"
    assert "S..." not in str(envelope)
    assert registry.unprotect(envelope, credential)["secret"] == "S..."
    with pytest.raises(InvalidPasswordError):
        registry.unprotect(envelope, ProtectionCredential.password("wrong"))


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


def test_system_provider_keeps_secret_encrypted_outside_key_store():
    key_store = MemorySystemKeyStore()
    registry = ProtectionRegistry([SystemProtectionProvider(key_store)])
    credential = ProtectionCredential.system()
    envelope = registry.protect({"kind": "secret", "secret": "S..."}, credential)

    assert registry.kind_for(envelope) == "system"
    assert "S..." not in str(envelope)
    assert len(key_store.keys) == 1
    assert registry.unprotect(envelope, credential)["secret"] == "S..."
    with pytest.raises(ProtectionError):
        registry.unprotect(envelope, ProtectionCredential.password("irrelevant"))


def test_wallet_manager_can_use_system_protection_without_password():
    key_store = MemorySystemKeyStore()
    registry = ProtectionRegistry([SystemProtectionProvider(key_store)])
    manager = WalletManager(MemoryWalletStorage(), registry)
    keypair = Keypair.random()

    record = manager.import_secret_with_protection(
        "system",
        keypair.secret,
        ProtectionCredential.system(),
    )

    assert manager.protection_kind("system") == "system"
    assert keypair.secret not in str(record.secret)
    session = manager.unlock_with("system", ProtectionCredential.system())
    assert session.wallet.address() == keypair.public_key


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

from copy import deepcopy

from stellar_sdk import Keypair

from fresnica.client_system_unlock import (
    SystemUnlockBackend,
    SystemUnlockController,
    SystemUnlockSlot,
    UnavailableSystemUnlockBackend,
)
from fresnica.manager import WalletManager
from fresnica.secret_store import WalletUnlockKey
from fresnica.storage import MemoryWalletStorage


class FakeSystemUnlockBackend(SystemUnlockBackend):
    label = "Fake system authentication"

    def __init__(self):
        self.values: dict[str, bytes] = {}
        self.release_count = 0

    def available(self) -> bool:
        return True

    def has(self, slot: SystemUnlockSlot) -> bool:
        return slot.storage_id in self.values

    def enroll(self, slot: SystemUnlockSlot, unlock_key: WalletUnlockKey) -> None:
        self.values[slot.storage_id] = bytes(unlock_key.as_bytes())

    def release(self, slot: SystemUnlockSlot) -> WalletUnlockKey:
        self.release_count += 1
        return WalletUnlockKey(self.values[slot.storage_id])

    def delete(self, slot: SystemUnlockSlot) -> None:
        self.values.pop(slot.storage_id, None)


def _manager() -> tuple[WalletManager, str]:
    manager = WalletManager(MemoryWalletStorage())
    keypair = Keypair.random()
    manager.import_secret("wallet", keypair.secret, "correct")
    return manager, keypair.public_key


def test_client_enrolls_and_releases_only_wallet_unlock_key():
    manager, public_key = _manager()
    backend = FakeSystemUnlockBackend()
    controller = SystemUnlockController(backend)

    controller.enroll(manager, "wallet", "correct")
    record = manager.get_record("wallet")

    assert controller.enrolled(record)
    assert len(next(iter(backend.values.values()))) == 32

    manager.lock()
    session = controller.unlock(manager, "wallet")

    assert session.wallet.address() == public_key
    assert backend.release_count == 1


def test_disable_removes_client_enrollment_without_touching_wallet_envelope():
    manager, _ = _manager()
    backend = FakeSystemUnlockBackend()
    controller = SystemUnlockController(backend)
    record = manager.get_record("wallet")
    original_envelope = deepcopy(record.secret)

    controller.enroll(manager, "wallet", "correct")
    controller.disable(manager, "wallet")

    assert not controller.enrolled(record)
    assert manager.get_record("wallet").secret == original_envelope
    assert manager.unlock("wallet", "correct").record.name == "wallet"


def test_enrollment_is_bound_to_exact_canonical_envelope():
    manager, _ = _manager()
    backend = FakeSystemUnlockBackend()
    controller = SystemUnlockController(backend)
    record = manager.get_record("wallet")

    controller.enroll(manager, "wallet", "correct")
    original_slot = SystemUnlockSlot.for_record(record)

    changed = deepcopy(record)
    changed.secret = deepcopy(record.secret)
    changed.secret["payload"]["nonce"] = "AAAAAAAAAAAAAAAA"
    changed_slot = SystemUnlockSlot.for_record(changed)

    assert original_slot.storage_id != changed_slot.storage_id
    assert controller.enrolled(record)
    assert not controller.enrolled(changed)


def test_unavailable_backend_never_claims_enrollment():
    manager, _ = _manager()
    controller = SystemUnlockController(UnavailableSystemUnlockBackend())

    assert not controller.available()
    assert not controller.enrolled(manager.get_record("wallet"))

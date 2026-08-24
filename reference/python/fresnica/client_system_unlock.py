"""Client-owned system authorization for software-wallet unlock keys.

This module intentionally contains no operating-system implementation. A TUI,
CLI, desktop app, or mobile client supplies a backend that protects and releases
``WalletUnlockKey`` values using its platform facilities. Core/reference wallet
cryptography stays unaware of Keychain, Keystore, biometrics, Hello, PAM, etc.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
import hashlib
import json

from .errors import FresnicaError
from .secret_store import WalletUnlockKey


class SystemUnlockError(FresnicaError):
    pass


class SystemUnlockUnavailableError(SystemUnlockError):
    pass


class SystemUnlockNotEnrolledError(SystemUnlockError):
    pass


@dataclass(frozen=True)
class SystemUnlockSlot:
    wallet_address: str
    envelope_fingerprint: str

    @property
    def storage_id(self) -> str:
        return f"{self.wallet_address}:{self.envelope_fingerprint}"

    @classmethod
    def for_record(cls, record) -> "SystemUnlockSlot":
        if record.watch_only or record.secret is None:
            raise SystemUnlockUnavailableError(
                "Watch-only wallet has no software unlock key"
            )
        encoded = json.dumps(
            record.secret,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode("utf-8")
        return cls(record.address, hashlib.sha256(encoded).hexdigest())


class SystemUnlockBackend(ABC):
    """OS/client adapter that stores and releases opaque wallet unlock keys."""

    label = "System authentication"

    @abstractmethod
    def available(self) -> bool:
        raise NotImplementedError

    @abstractmethod
    def has(self, slot: SystemUnlockSlot) -> bool:
        raise NotImplementedError

    @abstractmethod
    def enroll(self, slot: SystemUnlockSlot, unlock_key: WalletUnlockKey) -> None:
        raise NotImplementedError

    @abstractmethod
    def release(self, slot: SystemUnlockSlot) -> WalletUnlockKey:
        """Release a key after whatever OS authorization the backend requires."""
        raise NotImplementedError

    @abstractmethod
    def delete(self, slot: SystemUnlockSlot) -> None:
        raise NotImplementedError


class UnavailableSystemUnlockBackend(SystemUnlockBackend):
    def available(self) -> bool:
        return False

    def has(self, slot: SystemUnlockSlot) -> bool:
        return False

    def enroll(self, slot: SystemUnlockSlot, unlock_key: WalletUnlockKey) -> None:
        raise SystemUnlockUnavailableError("System unlock is unavailable on this client")

    def release(self, slot: SystemUnlockSlot) -> WalletUnlockKey:
        raise SystemUnlockUnavailableError("System unlock is unavailable on this client")

    def delete(self, slot: SystemUnlockSlot) -> None:
        return None


class SystemUnlockController:
    """Client orchestration between WalletManager and one OS backend."""

    def __init__(self, backend: SystemUnlockBackend):
        self.backend = backend

    def available(self) -> bool:
        return self.backend.available()

    def enrolled(self, record) -> bool:
        if not self.available() or record.watch_only or record.secret is None:
            return False
        return self.backend.has(SystemUnlockSlot.for_record(record))

    def enroll(self, manager, wallet_name: str, passcode: str) -> None:
        if not self.available():
            raise SystemUnlockUnavailableError("System unlock is unavailable on this client")
        record = manager.get_record(wallet_name)
        slot = SystemUnlockSlot.for_record(record)
        unlock_key = manager.derive_verified_unlock_key(wallet_name, passcode)
        self.backend.enroll(slot, unlock_key)

    def unlock(self, manager, wallet_name: str):
        if not self.available():
            raise SystemUnlockUnavailableError("System unlock is unavailable on this client")
        record = manager.get_record(wallet_name)
        slot = SystemUnlockSlot.for_record(record)
        if not self.backend.has(slot):
            raise SystemUnlockNotEnrolledError(
                f'System unlock is not enrolled for wallet "{record.name}"'
            )
        unlock_key = self.backend.release(slot)
        return manager.unlock_with_key(wallet_name, unlock_key)

    def disable(self, manager, wallet_name: str) -> None:
        if not self.available():
            raise SystemUnlockUnavailableError("System unlock is unavailable on this client")
        record = manager.get_record(wallet_name)
        self.backend.delete(SystemUnlockSlot.for_record(record))

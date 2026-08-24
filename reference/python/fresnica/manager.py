"""User-facing wallet lifecycle and session state management."""

from dataclasses import dataclass, replace
from datetime import datetime, timezone
from enum import Enum

from .errors import (
    InvalidPasswordError,
    InvalidUnlockKeyError,
    WalletError,
    WalletLockedError,
    WalletNotFoundError,
    WatchOnlyError,
)
from .hdwallet import detect_mnemonic_language, generate_mnemonic_phrase
from .network import get_network
from .protection import ProtectionCredential, ProtectionRegistry
from .secret_store import WalletUnlockKey
from .storage import WalletRecord, WalletStorage
from .wallet import Wallet
from .wallet_backup import read_wallet_backup, write_wallet_backup


@dataclass
class WalletSession:
    record: WalletRecord
    wallet: Wallet


class WalletState(str, Enum):
    WATCH_ONLY = "WATCH_ONLY"
    LOCKED = "LOCKED"
    UNLOCKED = "UNLOCKED"


@dataclass(frozen=True)
class WalletCapabilities:
    state: WalletState
    can_send: bool
    can_unlock: bool
    can_lock: bool
    can_fund_testnet: bool


class WalletManager:
    def __init__(
        self,
        storage: WalletStorage,
        protection_registry: ProtectionRegistry | None = None,
    ):
        self.storage = storage
        self.protection_registry = protection_registry or ProtectionRegistry()
        self._session: WalletSession | None = None

    def list_wallets(self) -> list[WalletRecord]:
        return self.storage.list()

    def get_record(self, name: str | None = None) -> WalletRecord:
        if name is None:
            name = self.storage.get_default()
            if name is None:
                records = self.storage.list()
                if len(records) == 1:
                    name = records[0].name
                else:
                    raise WalletNotFoundError("No default wallet selected")
        return self.storage.load(name)

    def set_default(self, name: str) -> None:
        self.storage.set_default(name)
        if self._session and self._session.record.name != name:
            self.lock()

    def state(self, name: str | None = None) -> WalletState:
        record = self.get_record(name)
        if record.watch_only:
            return WalletState.WATCH_ONLY
        if self._session and self._session.record.name == record.name:
            return WalletState.UNLOCKED
        return WalletState.LOCKED

    def capabilities(self, name: str | None = None) -> WalletCapabilities:
        record = self.get_record(name)
        state = self.state(record.name)
        return WalletCapabilities(
            state=state,
            can_send=state is not WalletState.WATCH_ONLY,
            can_unlock=state is WalletState.LOCKED,
            can_lock=state is WalletState.UNLOCKED,
            can_fund_testnet=record.network == "testnet",
        )

    def protection_kind(self, name: str | None = None) -> str | None:
        record = self.get_record(name)
        if record.watch_only or record.secret is None:
            return None
        return self.protection_registry.kind_for(record.secret)

    def add_watch(
        self,
        name: str,
        address: str,
        network: str = "mainnet",
        make_default: bool | None = None,
    ) -> WalletRecord:
        get_network(network)
        wallet = Wallet.from_address(address)
        record = WalletRecord(
            name=name,
            address=wallet.address(),
            wallet_type="watch-only",
            network=network,
            metadata={"created_at": _now()},
        )
        self.storage.save(record)
        self._maybe_default(record.name, make_default)
        return record

    def import_secret(
        self,
        name: str,
        secret: str,
        wallet_password: str,
        network: str = "mainnet",
        make_default: bool | None = None,
    ) -> WalletRecord:
        return self.import_secret_with_protection(
            name,
            secret,
            ProtectionCredential.password(wallet_password),
            network=network,
            make_default=make_default,
        )

    def import_secret_with_protection(
        self,
        name: str,
        secret: str,
        credential: ProtectionCredential,
        network: str = "mainnet",
        make_default: bool | None = None,
    ) -> WalletRecord:
        get_network(network)
        wallet = Wallet.from_secret(secret)
        envelope = self.protection_registry.protect(
            {"kind": "secret", "secret": secret.strip()},
            credential,
        )
        record = WalletRecord(
            name=name,
            address=wallet.address(),
            wallet_type="secret",
            network=network,
            secret=envelope,
            metadata={"created_at": _now()},
        )
        self.storage.save(record)
        self._maybe_default(record.name, make_default)
        return record

    def import_mnemonic(
        self,
        name: str,
        mnemonic: str,
        wallet_password: str,
        mnemonic_passphrase: str = "",
        index: int = 0,
        language=None,
        network: str = "mainnet",
        make_default: bool | None = None,
    ) -> WalletRecord:
        return self.import_mnemonic_with_protection(
            name,
            mnemonic,
            ProtectionCredential.password(wallet_password),
            mnemonic_passphrase=mnemonic_passphrase,
            index=index,
            language=language,
            network=network,
            make_default=make_default,
        )

    def import_mnemonic_with_protection(
        self,
        name: str,
        mnemonic: str,
        credential: ProtectionCredential,
        mnemonic_passphrase: str = "",
        index: int = 0,
        language=None,
        network: str = "mainnet",
        make_default: bool | None = None,
    ) -> WalletRecord:
        get_network(network)
        if language is None:
            language = detect_mnemonic_language(mnemonic)
        language_value = getattr(language, "value", language)
        wallet = Wallet.from_mnemonic(
            mnemonic,
            passphrase=mnemonic_passphrase,
            index=index,
            language=language,
        )
        envelope = self.protection_registry.protect(
            {
                "kind": "mnemonic",
                "mnemonic": mnemonic.strip(),
                "mnemonic_passphrase": mnemonic_passphrase,
                "index": index,
                "language": language_value,
            },
            credential,
        )
        record = WalletRecord(
            name=name,
            address=wallet.address(),
            wallet_type="mnemonic",
            network=network,
            secret=envelope,
            metadata={
                "created_at": _now(),
                "index": index,
                "language": language_value,
            },
        )
        self.storage.save(record)
        self._maybe_default(record.name, make_default)
        return record

    def create_mnemonic(
        self,
        name: str,
        wallet_password: str,
        mnemonic_passphrase: str = "",
        index: int = 0,
        language="english",
        strength: int = 256,
        network: str = "mainnet",
        make_default: bool | None = None,
    ) -> tuple[WalletRecord, str]:
        return self.create_mnemonic_with_protection(
            name,
            ProtectionCredential.password(wallet_password),
            mnemonic_passphrase=mnemonic_passphrase,
            index=index,
            language=language,
            strength=strength,
            network=network,
            make_default=make_default,
        )

    def create_mnemonic_with_protection(
        self,
        name: str,
        credential: ProtectionCredential,
        mnemonic_passphrase: str = "",
        index: int = 0,
        language="english",
        strength: int = 256,
        network: str = "mainnet",
        make_default: bool | None = None,
    ) -> tuple[WalletRecord, str]:
        mnemonic = generate_mnemonic_phrase(language=language, strength=strength)
        record = self.import_mnemonic_with_protection(
            name,
            mnemonic,
            credential,
            mnemonic_passphrase=mnemonic_passphrase,
            index=index,
            language=language,
            network=network,
            make_default=make_default,
        )
        return record, mnemonic

    def backup(self, name: str, path, overwrite: bool = False):
        """Back up the stored encrypted record without unlocking the wallet."""
        return write_wallet_backup(
            self.get_record(name),
            path,
            overwrite=overwrite,
        )

    def restore_backup(
        self,
        path,
        name: str | None = None,
        make_default: bool | None = None,
    ) -> WalletRecord:
        """Restore an encrypted record; protection validation remains an unlock concern."""
        record = read_wallet_backup(path)
        if name is not None:
            name = name.strip()
            if not name:
                raise WalletError("Restored wallet name cannot be empty")
            record = replace(record, name=name)
        self.storage.save(record)
        self._maybe_default(record.name, make_default)
        return record

    def view(self, name: str | None = None) -> WalletSession:
        record = self.get_record(name)
        return WalletSession(record, Wallet.from_address(record.address))

    def derive_verified_unlock_key(
        self,
        name: str | None,
        wallet_password: str,
    ) -> WalletUnlockKey:
        record = self._signing_record(name)
        unlock_key = self.protection_registry.derive_unlock_key(
            record.secret, wallet_password
        )
        try:
            payload = self.protection_registry.unprotect_with_unlock_key(
                record.secret, unlock_key
            )
        except InvalidUnlockKeyError as exc:
            raise InvalidPasswordError("Invalid wallet password") from exc
        wallet = self._wallet_from_payload(payload)
        self._verify_identity(record, wallet)
        return unlock_key

    def unlock(self, name: str | None, wallet_password: str) -> WalletSession:
        unlock_key = self.derive_verified_unlock_key(name, wallet_password)
        return self.unlock_with_key(name, unlock_key)

    def unlock_with_key(
        self,
        name: str | None,
        unlock_key: WalletUnlockKey,
    ) -> WalletSession:
        record = self._signing_record(name)
        payload = self.protection_registry.unprotect_with_unlock_key(
            record.secret, unlock_key
        )
        wallet = self._wallet_from_payload(payload)
        self._verify_identity(record, wallet)
        self._session = WalletSession(record, wallet)
        return self._session

    def unlock_with(
        self,
        name: str | None,
        credential: ProtectionCredential,
    ) -> WalletSession:
        """Compatibility entry point for the password-only reference API."""
        if credential.kind != "password" or not isinstance(credential.value, str):
            raise WalletLockedError("Core accepts only password protection credentials")
        return self.unlock(name, credential.value)

    def upgrade_legacy_protection(
        self,
        name: str | None,
        credential: ProtectionCredential,
    ) -> WalletRecord:
        """Wrap a pre-provider password envelope in protection metadata.

        This is a schema migration only: the encrypted payload and its password
        KDF remain byte-for-byte unchanged.
        """
        record = self.get_record(name)
        if record.watch_only or record.secret is None:
            return record
        if not self.protection_registry.is_legacy_password(record.secret):
            return record

        self.protection_registry.unprotect(record.secret, credential)
        upgraded = replace(
            record,
            secret=self.protection_registry.migrate_legacy_password(record.secret),
        )
        self.storage.save(upgraded, overwrite=True)
        if self._session and self._session.record.name == upgraded.name:
            self._session = WalletSession(upgraded, self._session.wallet)
        return upgraded

    def current(self) -> WalletSession | None:
        return self._session

    def lock(self) -> None:
        self._session = None

    close = lock

    def delete(self, name: str) -> None:
        was_default = self.storage.get_default() == name
        if self._session and self._session.record.name == name:
            self.lock()
        self.storage.delete(name)
        if was_default:
            records = self.storage.list()
            if records:
                self.storage.set_default(records[0].name)

    def _signing_record(self, name: str | None) -> WalletRecord:
        record = self.get_record(name)
        if record.watch_only:
            raise WatchOnlyError("Watch-only wallet cannot be unlocked for signing")
        if record.secret is None:
            raise WalletLockedError("Wallet has no protected signing material")
        return record

    @staticmethod
    def _wallet_from_payload(payload: dict) -> Wallet:
        kind = payload.get("kind")
        if kind == "secret":
            return Wallet.from_secret(payload["secret"])
        if kind == "mnemonic":
            return Wallet.from_mnemonic(
                payload["mnemonic"],
                passphrase=payload.get("mnemonic_passphrase", ""),
                index=int(payload.get("index", 0)),
                language=payload.get("language"),
            )
        raise WalletLockedError("Unsupported wallet signing material")

    @staticmethod
    def _verify_identity(record: WalletRecord, wallet: Wallet) -> None:
        if wallet.address() != record.address:
            raise WalletLockedError("Decrypted wallet identity does not match metadata")

    def _maybe_default(self, name: str, make_default: bool | None) -> None:
        if make_default is True or (
            make_default is None and self.storage.get_default() is None
        ):
            self.storage.set_default(name)


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()

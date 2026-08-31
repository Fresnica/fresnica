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
from .signer import FresnicaProcessProtectedSigner
from .storage import WalletRecord, WalletStorage
from .wallet import Account, Wallet
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
        core_client=None,
    ):
        self.storage = storage
        self.protection_registry = protection_registry or ProtectionRegistry()
        self.core_client = core_client
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
            can_unlock=state is WalletState.LOCKED and record.signer_kind == "protected-software",
            can_lock=state is WalletState.UNLOCKED,
            can_fund_testnet=record.network == "testnet",
        )

    def protection_kind(self, name: str | None = None) -> str | None:
        record = self.get_record(name)
        if record.signer_kind != "protected-software" or record.secret is None:
            return None
        return self.protection_registry.kind_for(record.secret)

    def has_app_passcode(self) -> bool:
        return any(
            record.signer_kind == "protected-software" and record.secret is not None
            for record in self.storage.list()
        )

    def validate_app_passcode(self, wallet_password: str) -> None:
        """Require one Fresnica passcode for every local protected software signer."""
        for record in self.storage.list():
            if record.signer_kind != "protected-software" or record.secret is None:
                continue
            try:
                self._derive_record_unlock_key(record, wallet_password)
            except InvalidPasswordError as exc:
                raise InvalidPasswordError("Invalid Fresnica passcode") from exc

    def add_watch(
        self,
        name: str,
        address: str,
        network: str = "mainnet",
        make_default: bool | None = None,
    ) -> WalletRecord:
        get_network(network)
        if self.core_client is not None:
            identity = self.core_client.parse_account(address)
            account = Account.parse(identity.address)
        else:
            account = Account.parse(address)
        record = WalletRecord(
            name=name,
            address=account.address,
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
        password = self._password_from_credential(credential)
        self.validate_app_passcode(password)
        if self.core_client is not None:
            protected = self.core_client.protect_secret(secret, password)
            signer_public_key = protected.signer_public_key
            envelope = protected.envelope
        else:
            wallet = Wallet.from_secret(secret)
            signer_public_key = wallet.signer_public_key()
            envelope = self.protection_registry.protect(
                {"kind": "secret", "secret": secret.strip()},
                credential,
            )
        record = WalletRecord(
            name=name,
            address=signer_public_key,
            wallet_type="secret",
            network=network,
            signer_kind="protected-software",
            signer_public_key=signer_public_key,
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
        password = self._password_from_credential(credential)
        self.validate_app_passcode(password)
        if language is None:
            language = detect_mnemonic_language(mnemonic)
        language_value = getattr(language, "value", language)
        if self.core_client is not None:
            protected = self.core_client.protect_mnemonic(
                mnemonic,
                password,
                mnemonic_passphrase=mnemonic_passphrase,
                index=index,
                language=language_value,
            )
            signer_public_key = protected.signer_public_key
            envelope = protected.envelope
        else:
            wallet = Wallet.from_mnemonic(
                mnemonic,
                passphrase=mnemonic_passphrase,
                index=index,
                language=language,
            )
            signer_public_key = wallet.signer_public_key()
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
            address=signer_public_key,
            wallet_type="mnemonic",
            network=network,
            signer_kind="protected-software",
            signer_public_key=signer_public_key,
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

    def upgrade_watch_with_secret(
        self,
        name: str,
        secret: str,
        wallet_password: str,
    ) -> WalletRecord:
        record = self._watch_classic_record(name)
        self.validate_app_passcode(wallet_password)
        if self.core_client is not None:
            protected = self.core_client.protect_secret(
                secret,
                wallet_password,
                expected_signer_public_key=record.address,
            )
            envelope = protected.envelope
            signer_public_key = protected.signer_public_key
        else:
            signer_wallet = Wallet.from_secret(secret)
            signer_public_key = signer_wallet.signer_public_key()
            if signer_public_key != record.address:
                raise WalletLockedError("Signer identity does not match watch-only account")
            envelope = self.protection_registry.protect(
                {"kind": "secret", "secret": secret.strip()},
                ProtectionCredential.password(wallet_password),
            )
        upgraded = replace(
            record,
            wallet_type="secret",
            signer_kind="protected-software",
            signer_public_key=signer_public_key,
            secret=envelope,
        )
        self.storage.save(upgraded, overwrite=True)
        return upgraded

    def upgrade_watch_with_mnemonic(
        self,
        name: str,
        mnemonic: str,
        wallet_password: str,
        mnemonic_passphrase: str = "",
        index: int = 0,
        language=None,
    ) -> WalletRecord:
        record = self._watch_classic_record(name)
        self.validate_app_passcode(wallet_password)
        if language is None:
            language = detect_mnemonic_language(mnemonic)
        language_value = getattr(language, "value", language)
        if self.core_client is not None:
            protected = self.core_client.protect_mnemonic(
                mnemonic,
                wallet_password,
                mnemonic_passphrase=mnemonic_passphrase,
                index=index,
                language=language_value,
                expected_signer_public_key=record.address,
            )
            envelope = protected.envelope
            signer_public_key = protected.signer_public_key
        else:
            signer_wallet = Wallet.from_mnemonic(
                mnemonic,
                passphrase=mnemonic_passphrase,
                index=index,
                language=language,
            )
            signer_public_key = signer_wallet.signer_public_key()
            if signer_public_key != record.address:
                raise WalletLockedError("Signer identity does not match watch-only account")
            envelope = self.protection_registry.protect(
                {
                    "kind": "mnemonic",
                    "mnemonic": mnemonic.strip(),
                    "mnemonic_passphrase": mnemonic_passphrase,
                    "index": index,
                    "language": language_value,
                },
                ProtectionCredential.password(wallet_password),
            )
        upgraded = replace(
            record,
            wallet_type="mnemonic",
            signer_kind="protected-software",
            signer_public_key=signer_public_key,
            secret=envelope,
            metadata={
                **record.metadata,
                "index": index,
                "language": language_value,
            },
        )
        self.storage.save(upgraded, overwrite=True)
        return upgraded

    def downgrade_to_watch(self, name: str) -> WalletRecord:
        """Remove local signer capability while preserving the account record.

        Caller/UI authorization policy is intentionally outside this reference
        lifecycle method.
        """
        record = self.get_record(name)
        downgraded = replace(
            record,
            wallet_type="watch-only",
            signer_kind=None,
            signer_public_key=None,
            secret=None,
        )
        if self._session and self._session.record.name == name:
            self.lock()
        self.storage.save(downgraded, overwrite=True)
        return downgraded

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
        if self.core_client is None:
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

        get_network(network)
        password = self._password_from_credential(credential)
        self.validate_app_passcode(password)
        language_value = getattr(language, "value", language)
        generated = self.core_client.generate_mnemonic(
            password,
            mnemonic_passphrase=mnemonic_passphrase,
            index=index,
            language=language_value,
            strength=strength,
        )
        record = WalletRecord(
            name=name,
            address=generated.signer_public_key,
            wallet_type="mnemonic",
            network=network,
            signer_kind="protected-software",
            signer_public_key=generated.signer_public_key,
            secret=generated.envelope,
            metadata={
                "created_at": _now(),
                "index": generated.index,
                "language": generated.language,
            },
        )
        self.storage.save(record)
        self._maybe_default(record.name, make_default)
        return record, generated.mnemonic

    def reprotect_signer(
        self,
        name: str | None,
        current_password: str,
        new_password: str,
    ) -> WalletRecord:
        """Replace one protected signer envelope without declassifying its secret."""
        record = self._signing_record(name)
        if self.core_client is not None:
            protected = self.core_client.reprotect(
                record.secret,
                current_password,
                new_password,
                record.signer_public_key,
            )
            envelope = protected.envelope
            signer_public_key = protected.signer_public_key
        else:
            payload = self.protection_registry.unprotect(
                record.secret,
                ProtectionCredential.password(current_password),
            )
            signer_wallet = self._wallet_from_payload(payload)
            self._verify_signer_identity(record, signer_wallet)
            envelope = self.protection_registry.protect(
                payload,
                ProtectionCredential.password(new_password),
            )
            signer_public_key = signer_wallet.signer_public_key()
        updated = replace(
            record,
            signer_public_key=signer_public_key,
            secret=envelope,
        )
        self.storage.save(updated, overwrite=True)
        if self._session and self._session.record.name == updated.name:
            self.lock()
        return updated

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
        wallet_password: str | None = None,
    ) -> WalletRecord:
        """Restore an encrypted record while preserving the app-passcode invariant."""
        record = read_wallet_backup(path)
        if name is not None:
            name = name.strip()
            if not name:
                raise WalletError("Restored wallet name cannot be empty")
            record = replace(record, name=name)
        if not record.watch_only and self.has_app_passcode():
            if not wallet_password:
                raise InvalidPasswordError(
                    "Fresnica passcode is required to restore a signing wallet"
                )
            self.validate_app_passcode(wallet_password)
            try:
                self._derive_record_unlock_key(record, wallet_password)
            except InvalidPasswordError as exc:
                raise InvalidPasswordError(
                    "Backup does not use the current Fresnica passcode"
                ) from exc
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
        return self._derive_record_unlock_key(record, wallet_password)

    def _derive_record_unlock_key(
        self,
        record: WalletRecord,
        wallet_password: str,
    ) -> WalletUnlockKey:
        if self.core_client is not None:
            return self.core_client.derive_verified_unlock_key(
                record.secret,
                wallet_password,
                record.signer_public_key,
            )

        unlock_key = self.protection_registry.derive_unlock_key(
            record.secret, wallet_password
        )
        try:
            payload = self.protection_registry.unprotect_with_unlock_key(
                record.secret, unlock_key
            )
        except InvalidUnlockKeyError as exc:
            raise InvalidPasswordError("Invalid wallet password") from exc
        signer_wallet = self._wallet_from_payload(payload)
        self._verify_signer_identity(record, signer_wallet)
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
        account_wallet = Wallet.from_address(record.address)
        if self.core_client is not None:
            self.core_client.validate_unlock_key(
                record.secret,
                unlock_key,
                record.signer_public_key,
            )
            signer = FresnicaProcessProtectedSigner(
                record.signer_public_key,
                self.core_client,
                record.secret,
                unlock_key,
            )
            wallet = account_wallet.with_signer(signer)
        else:
            payload = self.protection_registry.unprotect_with_unlock_key(
                record.secret, unlock_key
            )
            signer_wallet = self._wallet_from_payload(payload)
            self._verify_signer_identity(record, signer_wallet)
            wallet = account_wallet.with_signer(signer_wallet.signer)
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

    def export_signing_material(self, name: str | None, wallet_password: str) -> dict:
        record = self._signing_record(name)
        if self.core_client is not None:
            return self.core_client.reveal(
                record.secret,
                wallet_password,
                record.signer_public_key,
            )
        payload = self.protection_registry.unprotect(
            record.secret,
            ProtectionCredential.password(wallet_password),
        )
        signer_wallet = self._wallet_from_payload(payload)
        self._verify_signer_identity(record, signer_wallet)
        return payload

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
        if record.signer_kind != "protected-software" or record.secret is None:
            return record
        if not self.protection_registry.is_legacy_password(record.secret):
            return record

        password = self._password_from_credential(credential)
        if self.core_client is not None:
            self.core_client.derive_verified_unlock_key(
                record.secret,
                password,
                record.signer_public_key,
            )
        else:
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
            raise WatchOnlyError("Watch-only wallet has no local signer")
        if record.signer_kind != "protected-software":
            raise WalletLockedError("Signer is not a protected software signer")
        if record.secret is None or record.signer_public_key is None:
            raise WalletLockedError("Wallet has no protected signing material")
        return record

    def _watch_classic_record(self, name: str) -> WalletRecord:
        record = self.get_record(name)
        if not record.watch_only:
            raise WalletError("Wallet already has a local signer")
        account = Account.parse(record.address)
        if not account.is_classic:
            raise WalletError("Direct secret/mnemonic upgrade requires a classic G account")
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
    def _verify_signer_identity(record: WalletRecord, signer_wallet: Wallet) -> None:
        if signer_wallet.signer_public_key() != record.signer_public_key:
            raise WalletLockedError("Decrypted signer identity does not match metadata")

    @staticmethod
    def _password_from_credential(credential: ProtectionCredential) -> str:
        if credential.kind != "password" or not isinstance(credential.value, str):
            raise WalletLockedError("Core accepts only password protection credentials")
        return credential.value

    def _maybe_default(self, name: str, make_default: bool | None) -> None:
        if make_default is True or (
            make_default is None and self.storage.get_default() is None
        ):
            self.storage.set_default(name)


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()

"""User-facing wallet lifecycle management."""

from dataclasses import dataclass
from datetime import datetime, timezone

from .errors import WalletLockedError, WalletNotFoundError, WatchOnlyError
from .hdwallet import detect_mnemonic_language
from .network import get_network
from .secret_store import decrypt_secret, encrypt_secret
from .storage import WalletRecord, WalletStorage
from .wallet import Wallet


@dataclass
class WalletSession:
    record: WalletRecord
    wallet: Wallet


class WalletManager:
    def __init__(self, storage: WalletStorage):
        self.storage = storage
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
        get_network(network)
        wallet = Wallet.from_secret(secret)
        envelope = encrypt_secret(
            {"kind": "secret", "secret": secret.strip()},
            wallet_password,
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
        envelope = encrypt_secret(
            {
                "kind": "mnemonic",
                "mnemonic": mnemonic.strip(),
                "mnemonic_passphrase": mnemonic_passphrase,
                "index": index,
                "language": language_value,
            },
            wallet_password,
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

    def view(self, name: str | None = None) -> WalletSession:
        record = self.get_record(name)
        return WalletSession(record, Wallet.from_address(record.address))

    def unlock(self, name: str | None, wallet_password: str) -> WalletSession:
        record = self.get_record(name)
        if record.watch_only:
            raise WatchOnlyError("Watch-only wallet cannot be unlocked for signing")
        if record.secret is None:
            raise WalletLockedError("Wallet has no encrypted signing material")

        payload = decrypt_secret(record.secret, wallet_password)
        kind = payload.get("kind")
        if kind == "secret":
            wallet = Wallet.from_secret(payload["secret"])
        elif kind == "mnemonic":
            wallet = Wallet.from_mnemonic(
                payload["mnemonic"],
                passphrase=payload.get("mnemonic_passphrase", ""),
                index=int(payload.get("index", 0)),
                language=payload.get("language"),
            )
        else:
            raise WalletLockedError("Unsupported wallet signing material")

        if wallet.address() != record.address:
            raise WalletLockedError("Decrypted wallet identity does not match metadata")
        self._session = WalletSession(record, wallet)
        return self._session

    def current(self) -> WalletSession | None:
        return self._session

    def lock(self) -> None:
        self._session = None

    close = lock

    def delete(self, name: str) -> None:
        if self._session and self._session.record.name == name:
            self.lock()
        self.storage.delete(name)

    def _maybe_default(self, name: str, make_default: bool | None) -> None:
        if make_default is True or (
            make_default is None and self.storage.get_default() is None
        ):
            self.storage.set_default(name)


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()

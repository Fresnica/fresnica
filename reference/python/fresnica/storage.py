"""Wallet metadata persistence.

Public account metadata stays readable. Local signer capability is represented
separately from the account address. Sensitive mnemonic or secret-key material
is stored only as an authenticated encrypted Core/reference envelope.
"""

from abc import ABC, abstractmethod
from dataclasses import asdict, dataclass, field
import hashlib
import json
import os
from pathlib import Path

from .errors import WalletExistsError, WalletNotFoundError


@dataclass
class WalletRecord:
    name: str
    address: str
    wallet_type: str
    network: str = "mainnet"
    secret: dict | None = None
    metadata: dict = field(default_factory=dict)
    signer_kind: str | None = None
    signer_public_key: str | None = None

    @property
    def watch_only(self) -> bool:
        return self.signer_kind is None

    @property
    def locked(self) -> bool:
        return self.signer_kind == "protected-software" and self.secret is not None

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, data: dict) -> "WalletRecord":
        # Pre-release compatibility for reference records created before signer
        # identity was separated from account identity.
        signer_kind = data.get("signer_kind")
        signer_public_key = data.get("signer_public_key")
        if signer_kind is None and data.get("wallet_type") != "watch-only" and data.get("secret") is not None:
            signer_kind = "protected-software"
            signer_public_key = signer_public_key or data["address"]
        return cls(
            name=data["name"],
            address=data["address"],
            wallet_type=data["wallet_type"],
            network=data.get("network", "mainnet"),
            secret=data.get("secret"),
            metadata=data.get("metadata") or {},
            signer_kind=signer_kind,
            signer_public_key=signer_public_key,
        )


class WalletStorage(ABC):
    @abstractmethod
    def save(self, wallet: WalletRecord, overwrite: bool = False):
        raise NotImplementedError

    @abstractmethod
    def load(self, name: str) -> WalletRecord:
        raise NotImplementedError

    @abstractmethod
    def list(self) -> list[WalletRecord]:
        raise NotImplementedError

    @abstractmethod
    def delete(self, name: str) -> None:
        raise NotImplementedError

    @abstractmethod
    def get_default(self) -> str | None:
        raise NotImplementedError

    @abstractmethod
    def set_default(self, name: str) -> None:
        raise NotImplementedError


class MemoryWalletStorage(WalletStorage):
    def __init__(self):
        self._wallets: dict[str, WalletRecord] = {}
        self._default: str | None = None

    def save(self, wallet: WalletRecord, overwrite: bool = False):
        if wallet.name in self._wallets and not overwrite:
            raise WalletExistsError(f"Wallet already exists: {wallet.name}")
        self._wallets[wallet.name] = wallet

    def load(self, name: str) -> WalletRecord:
        try:
            return self._wallets[name]
        except KeyError as exc:
            raise WalletNotFoundError(f"Wallet not found: {name}") from exc

    def list(self) -> list[WalletRecord]:
        return sorted(self._wallets.values(), key=lambda item: item.name.lower())

    def delete(self, name: str) -> None:
        if name not in self._wallets:
            raise WalletNotFoundError(f"Wallet not found: {name}")
        del self._wallets[name]
        if self._default == name:
            self._default = None

    def get_default(self) -> str | None:
        return self._default

    def set_default(self, name: str) -> None:
        self.load(name)
        self._default = name


class FileWalletStorage(WalletStorage):
    """One JSON file per wallet plus a tiny default-wallet pointer."""

    def __init__(self, directory: str | Path):
        self.directory = Path(directory).expanduser()
        self.directory.mkdir(parents=True, exist_ok=True)
        try:
            os.chmod(self.directory, 0o700)
        except OSError:
            pass
        self._default_path = self.directory / ".default"

    def _path(self, name: str) -> Path:
        digest = hashlib.sha256(name.encode("utf-8")).hexdigest()
        return self.directory / f"{digest}.wallet.json"

    def _atomic_write(self, path: Path, text: str) -> None:
        temporary = path.with_suffix(path.suffix + ".tmp")
        temporary.write_text(text, encoding="utf-8")
        try:
            os.chmod(temporary, 0o600)
        except OSError:
            pass
        os.replace(temporary, path)

    def save(self, wallet: WalletRecord, overwrite: bool = False):
        path = self._path(wallet.name)
        if path.exists() and not overwrite:
            raise WalletExistsError(f"Wallet already exists: {wallet.name}")
        self._atomic_write(
            path,
            json.dumps(wallet.to_dict(), ensure_ascii=False, indent=2) + "\n",
        )

    def load(self, name: str) -> WalletRecord:
        path = self._path(name)
        if not path.exists():
            raise WalletNotFoundError(f"Wallet not found: {name}")
        data = json.loads(path.read_text(encoding="utf-8"))
        record = WalletRecord.from_dict(data)
        if record.name != name:
            raise WalletNotFoundError(f"Wallet not found: {name}")
        return record

    def list(self) -> list[WalletRecord]:
        records = []
        for path in self.directory.glob("*.wallet.json"):
            data = json.loads(path.read_text(encoding="utf-8"))
            records.append(WalletRecord.from_dict(data))
        return sorted(records, key=lambda item: item.name.lower())

    def delete(self, name: str) -> None:
        path = self._path(name)
        if not path.exists():
            raise WalletNotFoundError(f"Wallet not found: {name}")
        path.unlink()
        if self.get_default() == name:
            self._default_path.unlink(missing_ok=True)

    def get_default(self) -> str | None:
        if not self._default_path.exists():
            return None
        value = self._default_path.read_text(encoding="utf-8").strip()
        return value or None

    def set_default(self, name: str) -> None:
        self.load(name)
        self._atomic_write(self._default_path, name + "\n")

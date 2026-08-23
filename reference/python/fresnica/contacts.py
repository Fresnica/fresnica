"""Local Stellar contact/address book persistence."""

from __future__ import annotations

from dataclasses import asdict, dataclass
import json
import os
from pathlib import Path

from .errors import FresnicaError
from .wallet import Wallet


class ContactError(FresnicaError):
    pass


class ContactNotFoundError(ContactError):
    pass


class ContactExistsError(ContactError):
    pass


@dataclass(frozen=True)
class Contact:
    name: str
    address: str
    memo: str | None = None


@dataclass(frozen=True)
class ResolvedDestination:
    address: str
    memo: str | None = None
    contact_name: str | None = None


def resolve_destination(
    store: "ContactStore",
    destination: str,
    memo: str | None = None,
) -> ResolvedDestination:
    """Resolve a local contact alias while keeping explicit memo precedence."""
    contact = store.find(destination)
    if contact is None:
        return ResolvedDestination(address=destination, memo=memo)
    return ResolvedDestination(
        address=contact.address,
        memo=memo if memo is not None else contact.memo,
        contact_name=contact.name,
    )


class ContactStore:
    def __init__(self, path: str | Path):
        self.path = Path(path).expanduser()

    def list(self) -> list[Contact]:
        return sorted(self._load(), key=lambda item: item.name.casefold())

    def get(self, name: str) -> Contact:
        key = _name_key(name)
        for contact in self._load():
            if contact.name.casefold() == key:
                return contact
        raise ContactNotFoundError(f"Contact not found: {name}")

    def find(self, name: str) -> Contact | None:
        try:
            return self.get(name)
        except ContactNotFoundError:
            return None

    def add(self, name: str, address: str, memo: str | None = None) -> Contact:
        contact = _contact(name, address, memo)
        items = self._load()
        if any(item.name.casefold() == contact.name.casefold() for item in items):
            raise ContactExistsError(f"Contact already exists: {contact.name}")
        items.append(contact)
        self._save(items)
        return contact

    def remove(self, name: str) -> Contact:
        contact = self.get(name)
        remaining = [
            item for item in self._load() if item.name.casefold() != contact.name.casefold()
        ]
        self._save(remaining)
        return contact

    def _load(self) -> list[Contact]:
        if not self.path.exists():
            return []
        try:
            raw = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError, TypeError) as exc:
            raise ContactError(f"Unable to read contacts: {self.path}") from exc
        if not isinstance(raw, list):
            raise ContactError("Contact address book is malformed")

        contacts = []
        seen = set()
        for value in raw:
            if not isinstance(value, dict):
                raise ContactError("Contact address book is malformed")
            try:
                contact = _contact(
                    value["name"],
                    value["address"],
                    value.get("memo"),
                )
            except (KeyError, TypeError, ValueError, ContactError) as exc:
                raise ContactError("Contact address book is malformed") from exc
            key = contact.name.casefold()
            if key in seen:
                raise ContactError("Contact address book contains duplicate names")
            seen.add(key)
            contacts.append(contact)
        return contacts

    def _save(self, contacts: list[Contact]) -> None:
        temporary = self.path.with_suffix(self.path.suffix + ".tmp")
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary.write_text(
                json.dumps(
                    [asdict(item) for item in contacts],
                    ensure_ascii=False,
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
            try:
                os.chmod(temporary, 0o600)
            except OSError:
                pass
            os.replace(temporary, self.path)
        except OSError as exc:
            temporary.unlink(missing_ok=True)
            raise ContactError(f"Unable to write contacts: {self.path}") from exc


def _name_key(name: str) -> str:
    if not isinstance(name, str) or not name.strip():
        raise ContactError("Contact name cannot be empty")
    return name.strip().casefold()


def _contact(name: str, address: str, memo: str | None) -> Contact:
    if not isinstance(name, str) or not name.strip():
        raise ContactError("Contact name cannot be empty")
    name = name.strip()
    if not isinstance(address, str):
        raise ContactError("Contact address is invalid")
    try:
        canonical = Wallet.from_address(address.strip()).address()
    except (TypeError, ValueError) as exc:
        raise ContactError("Contact address is not a valid Stellar G... account") from exc
    if memo is not None:
        if not isinstance(memo, str):
            raise ContactError("Contact memo must be text")
        memo = memo.strip() or None
    return Contact(name=name, address=canonical, memo=memo)

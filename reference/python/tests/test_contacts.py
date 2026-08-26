import json

import pytest
from stellar_sdk import Keypair

from fresnica.contacts import (
    ContactError,
    ContactExistsError,
    ContactNotFoundError,
    ContactStore,
    resolve_destination,
)


def test_contact_store_persists_case_insensitive_names_and_default_memo(tmp_path):
    path = tmp_path / "contacts.json"
    address = Keypair.random().public_key
    store = ContactStore(path)

    contact = store.add("Alice", address, memo="12345")

    assert contact.name == "Alice"
    assert contact.address == address
    assert contact.memo == "12345"
    assert ContactStore(path).get("alice") == contact
    assert ContactStore(path).find_by_address(address) == contact
    assert json.loads(path.read_text(encoding="utf-8")) == [
        {"address": address, "memo": "12345", "name": "Alice"}
    ]

    with pytest.raises(ContactExistsError):
        store.add("ALICE", Keypair.random().public_key)


def test_contact_store_remove_and_missing_contact(tmp_path):
    store = ContactStore(tmp_path / "contacts.json")
    contact = store.add("Bob", Keypair.random().public_key)

    removed = store.remove("bob")

    assert removed == contact
    assert store.list() == []
    assert store.find("bob") is None
    assert store.find_by_address(contact.address) is None
    with pytest.raises(ContactNotFoundError):
        store.get("bob")


def test_contact_store_rejects_invalid_address_and_corrupt_file(tmp_path):
    store = ContactStore(tmp_path / "contacts.json")
    with pytest.raises(ContactError, match="valid Stellar"):
        store.add("bad", "not-an-address")
    assert store.find_by_address("not-an-address") is None

    store.path.write_text("not-json", encoding="utf-8")
    with pytest.raises(ContactError, match="Unable to read contacts"):
        store.list()


def test_resolve_destination_preserves_contact_identity_and_memo_precedence(tmp_path):
    address = Keypair.random().public_key
    store = ContactStore(tmp_path / "contacts.json")
    store.add("Alice", address, memo="default-memo")

    default = resolve_destination(store, "alice")
    assert default.address == address
    assert default.contact_name == "Alice"
    assert default.memo == "default-memo"

    explicit = resolve_destination(store, "ALICE", "explicit-memo")
    assert explicit.address == address
    assert explicit.contact_name == "Alice"
    assert explicit.memo == "explicit-memo"

    raw = Keypair.random().public_key
    direct = resolve_destination(store, raw, "direct-memo")
    assert direct.address == raw
    assert direct.contact_name is None
    assert direct.memo == "direct-memo"

    no_store = resolve_destination(None, raw)
    assert no_store.address == raw
    assert no_store.contact_name is None
    assert no_store.memo is None

    shadowed = Keypair.random().public_key
    store.add(raw, shadowed, memo="shadowed-memo")
    direct_over_alias = resolve_destination(store, raw, "direct-memo")
    assert direct_over_alias.address == raw
    assert direct_over_alias.contact_name is None
    assert direct_over_alias.memo == "direct-memo"

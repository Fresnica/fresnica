import os
from pathlib import Path
import subprocess

import pytest

from fresnica.contacts import ContactStore
from fresnica.manager import WalletManager
from fresnica.storage import FileWalletStorage
from fresnica.wallet_backup import read_wallet_backup, write_wallet_backup


SECRET = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO"
PUBLIC = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR"
SECOND_PUBLIC = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"


@pytest.fixture(scope="module")
def rust_cli():
    binary = os.environ.get("FRESNICA_RUST_CLI_BIN")
    if not binary:
        pytest.skip("Native Rust CLI integration binary is not configured")
    return Path(binary)


def _run(binary: Path, home: Path, *args: str):
    completed = subprocess.run(
        [str(binary), "--home", str(home), *args],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    return completed.stdout


def test_rust_cli_reads_python_wallet_storage_and_writes_python_backup(rust_cli, tmp_path):
    home = tmp_path / "shared-home"
    storage = FileWalletStorage(home / "wallets")
    manager = WalletManager(storage)
    record = manager.import_secret(
        "python-wallet",
        SECRET,
        "passcode",
        network="testnet",
        make_default=True,
    )

    listed = _run(rust_cli, home, "wallet", "list")
    assert "python-wallet" in listed
    assert PUBLIC in listed
    assert "testnet" in listed

    info = _run(rust_cli, home, "info", "--wallet", "python-wallet")
    assert "SDK/Core:   Rust (direct link)" in info
    assert "Fresnica passcode envelope v1" in info
    assert PUBLIC in info

    backup_path = tmp_path / "rust-written.backup.json"
    _run(rust_cli, home, "wallet", "backup", "python-wallet", str(backup_path))
    restored = read_wallet_backup(backup_path)
    assert restored.name == record.name
    assert restored.address == record.address
    assert restored.secret == record.secret


def test_python_reads_rust_cli_watch_wallet_and_default_pointer(rust_cli, tmp_path):
    home = tmp_path / "rust-home"
    _run(
        rust_cli,
        home,
        "--network",
        "testnet",
        "wallet",
        "import-watch",
        "rust-watch",
        PUBLIC,
    )

    storage = FileWalletStorage(home / "wallets")
    record = storage.load("rust-watch")
    assert record.address == PUBLIC
    assert record.network == "testnet"
    assert record.watch_only
    assert storage.get_default() == "rust-watch"


def test_rust_cli_restores_python_backup_without_changing_envelope(rust_cli, tmp_path):
    source_home = tmp_path / "source"
    source_storage = FileWalletStorage(source_home / "wallets")
    source_manager = WalletManager(source_storage)
    original = source_manager.import_secret(
        "portable",
        SECRET,
        "passcode",
        network="mainnet",
        make_default=True,
    )
    backup_path = tmp_path / "python-written.backup.json"
    write_wallet_backup(original, backup_path)

    destination_home = tmp_path / "destination"
    _run(
        rust_cli,
        destination_home,
        "wallet",
        "restore",
        str(backup_path),
        "--name",
        "restored-by-rust",
    )

    destination_storage = FileWalletStorage(destination_home / "wallets")
    restored = destination_storage.load("restored-by-rust")
    assert restored.address == original.address
    assert restored.network == original.network
    assert restored.secret == original.secret
    assert destination_storage.get_default() == "restored-by-rust"


def test_rust_cli_reads_python_contacts(rust_cli, tmp_path):
    home = tmp_path / "python-contacts"
    ContactStore(home / "contacts.json").add("Alice", PUBLIC, memo="default-memo")

    listed = _run(rust_cli, home, "contact", "list")

    assert "Alice" in listed
    assert PUBLIC in listed
    assert "default-memo" in listed


def test_python_reads_rust_cli_contacts(rust_cli, tmp_path):
    home = tmp_path / "rust-contacts"
    _run(
        rust_cli,
        home,
        "contact",
        "add",
        "Bob",
        SECOND_PUBLIC,
        "--memo",
        "12345",
    )

    contact = ContactStore(home / "contacts.json").get("bob")
    assert contact.name == "Bob"
    assert contact.address == SECOND_PUBLIC
    assert contact.memo == "12345"

import json
import os
import stat

import pytest
from stellar_sdk import Keypair

from fresnica.errors import WalletError
from fresnica.manager import WalletManager
from fresnica.storage import MemoryWalletStorage
from fresnica.wallet_backup import BACKUP_FORMAT, BACKUP_VERSION, read_wallet_backup


MNEMONIC = "illness spike retreat truth genius clock brain pass fit cave bargain toe"


def test_encrypted_mnemonic_backup_round_trips_without_exposing_plaintext(tmp_path):
    source = WalletManager(MemoryWalletStorage())
    original = source.import_mnemonic(
        "primary",
        MNEMONIC,
        "correct horse battery staple",
        network="testnet",
    )
    path = tmp_path / "primary.fresnica-wallet.json"

    written = source.backup("primary", path)

    assert written == path
    raw_text = path.read_text(encoding="utf-8")
    raw = json.loads(raw_text)
    assert raw["format"] == BACKUP_FORMAT
    assert raw["version"] == BACKUP_VERSION
    assert raw["wallet"]["address"] == original.address
    assert raw["wallet"]["secret"] == original.secret
    assert MNEMONIC not in raw_text
    assert "correct horse battery staple" not in raw_text
    if os.name == "posix":
        assert stat.S_IMODE(path.stat().st_mode) == 0o600

    restored_manager = WalletManager(MemoryWalletStorage())
    restored = restored_manager.restore_backup(path, name="restored")
    session = restored_manager.unlock("restored", "correct horse battery staple")

    assert restored.name == "restored"
    assert restored.address == original.address
    assert restored.network == "testnet"
    assert restored.secret == original.secret
    assert session.wallet.address() == original.address


def test_watch_only_backup_round_trips_without_signing_material(tmp_path):
    address = Keypair.random().public_key
    source = WalletManager(MemoryWalletStorage())
    source.add_watch("observer", address, network="mainnet")
    path = tmp_path / "observer.json"

    source.backup("observer", path)
    restored = WalletManager(MemoryWalletStorage()).restore_backup(path)

    assert restored.name == "observer"
    assert restored.address == address
    assert restored.watch_only
    assert restored.secret is None


def test_backup_refuses_to_overwrite_unless_explicit(tmp_path):
    source = WalletManager(MemoryWalletStorage())
    source.add_watch("observer", Keypair.random().public_key)
    path = tmp_path / "observer.json"
    source.backup("observer", path)

    with pytest.raises(WalletError, match="already exists"):
        source.backup("observer", path)

    source.backup("observer", path, overwrite=True)
    assert read_wallet_backup(path).name == "observer"


def test_restore_rejects_unsupported_or_malformed_backup(tmp_path):
    unsupported = tmp_path / "future.json"
    unsupported.write_text(
        json.dumps({"format": BACKUP_FORMAT, "version": 999, "wallet": {}}),
        encoding="utf-8",
    )
    with pytest.raises(WalletError, match="Unsupported wallet backup format"):
        read_wallet_backup(unsupported)

    malformed = tmp_path / "malformed.json"
    malformed.write_text(
        json.dumps(
            {
                "format": BACKUP_FORMAT,
                "version": BACKUP_VERSION,
                "wallet": {
                    "name": "bad",
                    "address": "not-a-stellar-address",
                    "wallet_type": "watch-only",
                    "network": "mainnet",
                    "secret": None,
                    "metadata": {},
                },
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(WalletError, match="invalid Stellar address"):
        read_wallet_backup(malformed)


def test_restore_rejects_unknown_network_as_backup_error(tmp_path):
    path = tmp_path / "unknown-network.json"
    path.write_text(
        json.dumps(
            {
                "format": BACKUP_FORMAT,
                "version": BACKUP_VERSION,
                "wallet": {
                    "name": "observer",
                    "address": Keypair.random().public_key,
                    "wallet_type": "watch-only",
                    "network": "future-net",
                    "secret": None,
                    "metadata": {},
                },
            }
        ),
        encoding="utf-8",
    )

    with pytest.raises(WalletError, match="unknown network future-net"):
        read_wallet_backup(path)


def test_restore_rejects_watch_only_record_with_secret_envelope(tmp_path):
    path = tmp_path / "bad-watch.json"
    path.write_text(
        json.dumps(
            {
                "format": BACKUP_FORMAT,
                "version": BACKUP_VERSION,
                "wallet": {
                    "name": "observer",
                    "address": Keypair.random().public_key,
                    "wallet_type": "watch-only",
                    "network": "mainnet",
                    "secret": {"ciphertext": "should-not-exist"},
                    "metadata": {},
                },
            }
        ),
        encoding="utf-8",
    )

    with pytest.raises(WalletError, match="watch-only wallet contains signing material"):
        read_wallet_backup(path)

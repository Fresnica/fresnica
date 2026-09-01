import json
import os
from pathlib import Path
from types import SimpleNamespace

import pytest
from stellar_sdk import Keypair, StrKey, TransactionEnvelope

from fresnica.cli.commands.tui import core_subtitle
from fresnica.errors import InvalidPasswordError, InvalidUnlockKeyError, WalletLockedError
from fresnica.manager import WalletManager
from fresnica.process_client import FresnicaProcessClient
from fresnica.secret_store import WalletUnlockKey
from fresnica.signer import FresnicaProcessProtectedSigner
from fresnica.storage import MemoryWalletStorage


VECTOR_PATH = (
    Path(__file__).parents[3]
    / "spec"
    / "test-vectors"
    / "transaction-signing-v1.json"
)
SOROBAN_VECTOR_PATH = (
    Path(__file__).parents[3]
    / "spec"
    / "test-vectors"
    / "soroban-authorization-signing-v1.json"
)


@pytest.fixture(scope="module")
def core_client():
    binary = os.environ.get("FRESNICA_PROCESS_BIN")
    if not binary:
        pytest.skip("Rust Core integration binary is not configured")
    return FresnicaProcessClient(binary)


def _vector():
    return json.loads(VECTOR_PATH.read_text(encoding="utf-8"))["cases"][0]


def _soroban_vector():
    return json.loads(SOROBAN_VECTOR_PATH.read_text(encoding="utf-8"))["cases"][0]


def test_tui_core_subtitle_reports_python_reference_without_bridge():
    runtime = SimpleNamespace(core_client=None)
    assert core_subtitle(runtime) == "Stellar Wallet · Python Reference"


def test_process_binding_version(core_client):
    version = core_client.version()
    assert version["protocol_version"] == 2
    assert version["client_api_version"] == 4
    assert version["process_binding_version"] == "0.2.0"
    assert version["process_binding_api_version"] == 2
    assert version["sdk_api_version"] == 4


def test_bridge_parses_classic_and_contract_identity(core_client):
    vector = _vector()
    classic = core_client.parse_account(vector["public_key"])
    assert classic.kind == "classic"
    assert classic.public_key == vector["public_key"]

    contract_address = StrKey.encode_contract(bytes(range(32)))
    contract = core_client.parse_account(contract_address)
    assert contract.kind == "contract"
    assert contract.address == contract_address
    assert contract.public_key is None


def test_tui_core_subtitle_reports_rust_bridge(core_client):
    runtime = SimpleNamespace(core_client=core_client)
    assert core_subtitle(runtime) == "Stellar Wallet · Rust Core"


def test_secret_roundtrip_signs_exact_shared_vector(core_client):
    vector = _vector()
    protected = core_client.protect_secret(vector["secret"], "passcode")

    assert protected.signer_public_key == vector["public_key"]
    assert vector["secret"] not in json.dumps(protected.envelope)

    unlock_key = core_client.derive_verified_unlock_key(
        protected.envelope,
        "passcode",
        protected.signer_public_key,
    )
    core_client.validate_unlock_key(
        protected.envelope,
        unlock_key,
        protected.signer_public_key,
    )
    signed_xdr = core_client.sign_transaction(
        protected.envelope,
        unlock_key,
        protected.signer_public_key,
        vector["unsigned_xdr_base64"],
        vector["network_passphrase"],
    )
    assert signed_xdr == vector["signed_xdr_base64"]

    revealed = core_client.reveal(
        protected.envelope,
        "passcode",
        protected.signer_public_key,
    )
    assert revealed == {"kind": "secret", "secret": vector["secret"]}


def test_watch_only_attachment_checks_expected_signer(core_client):
    vector = _vector()
    protected = core_client.protect_secret(
        vector["secret"],
        "passcode",
        expected_signer_public_key=vector["public_key"],
    )
    assert protected.signer_public_key == vector["public_key"]

    with pytest.raises(WalletLockedError):
        core_client.protect_secret(
            vector["secret"],
            "passcode",
            expected_signer_public_key=Keypair.random().public_key,
        )


def test_reprotect_rotates_unlock_key_and_passcode(core_client):
    vector = _vector()
    protected = core_client.protect_secret(vector["secret"], "old")
    old_key = core_client.derive_verified_unlock_key(
        protected.envelope,
        "old",
        protected.signer_public_key,
    )

    changed = core_client.reprotect(
        protected.envelope,
        "old",
        "new",
        protected.signer_public_key,
    )
    new_key = core_client.derive_verified_unlock_key(
        changed.envelope,
        "new",
        changed.signer_public_key,
    )

    assert changed.signer_public_key == protected.signer_public_key
    assert changed.envelope != protected.envelope
    assert new_key.as_bytes() != old_key.as_bytes()
    with pytest.raises(InvalidPasswordError):
        core_client.derive_verified_unlock_key(
            changed.envelope,
            "old",
            changed.signer_public_key,
        )


def test_external_signing_prepare_and_apply_matches_shared_vector(core_client):
    vector = _vector()
    prepared = core_client.prepare_ed25519_signing(
        vector["unsigned_xdr_base64"],
        vector["network_passphrase"],
    )
    signature = Keypair.from_secret(vector["secret"]).sign(prepared.transaction_hash)
    signed = core_client.apply_ed25519_signature(
        prepared.transaction_xdr,
        prepared.network_passphrase,
        vector["public_key"],
        signature,
    )
    assert signed == vector["signed_xdr_base64"]


def test_soroban_authorization_protected_and_passcode_paths_match_shared_vector(core_client):
    vector = _soroban_vector()
    protected = core_client.protect_secret(vector["secret"], "passcode")
    unlock_key = core_client.derive_verified_unlock_key(
        protected.envelope,
        "passcode",
        protected.signer_public_key,
    )

    signed = core_client.sign_soroban_authorization(
        protected.envelope,
        unlock_key,
        protected.signer_public_key,
        vector["unsigned_entry_xdr_base64"],
        vector["network_passphrase"],
    )
    assert signed == vector["signed_entry_xdr_base64"]

    passcode_signed = core_client.sign_soroban_authorization_with_passcode(
        protected.envelope,
        "passcode",
        protected.signer_public_key,
        vector["unsigned_entry_xdr_base64"],
        vector["network_passphrase"],
    )
    assert passcode_signed == vector["signed_entry_xdr_base64"]


def test_soroban_authorization_external_prepare_apply_matches_shared_vector(core_client):
    vector = _soroban_vector()
    prepared = core_client.prepare_soroban_authorization_signing(
        vector["unsigned_entry_xdr_base64"],
        vector["network_passphrase"],
    )
    assert prepared.authorization_hash.hex() == vector["authorization_hash_hex"]
    assert prepared.authorization_entry_xdr == vector["unsigned_entry_xdr_base64"]
    assert prepared.authorization_preimage_xdr == vector["authorization_preimage_xdr_base64"]
    assert prepared.network_passphrase == vector["network_passphrase"]

    signature = Keypair.from_secret(vector["secret"]).sign(prepared.authorization_hash)
    signed = core_client.apply_soroban_ed25519_signature(
        prepared.authorization_entry_xdr,
        prepared.network_passphrase,
        vector["public_key"],
        signature,
    )
    assert signed == vector["signed_entry_xdr_base64"]


def test_same_passcode_unlock_keys_are_bound_to_each_envelope(core_client):
    vector = _vector()
    first = core_client.protect_secret(vector["secret"], "shared-passcode")
    second = core_client.protect_secret(vector["secret"], "shared-passcode")

    first_key = core_client.derive_verified_unlock_key(
        first.envelope,
        "shared-passcode",
        first.signer_public_key,
    )
    second_key = core_client.derive_verified_unlock_key(
        second.envelope,
        "shared-passcode",
        second.signer_public_key,
    )

    assert first_key.as_bytes() != second_key.as_bytes()
    with pytest.raises(InvalidUnlockKeyError):
        core_client.validate_unlock_key(
            second.envelope,
            first_key,
            second.signer_public_key,
        )
    with pytest.raises(InvalidUnlockKeyError):
        core_client.validate_unlock_key(
            first.envelope,
            second_key,
            first.signer_public_key,
        )


def test_wrong_passcode_and_unlock_key_fail_closed(core_client):
    vector = _vector()
    protected = core_client.protect_secret(vector["secret"], "correct")

    with pytest.raises(InvalidPasswordError):
        core_client.derive_verified_unlock_key(
            protected.envelope,
            "wrong",
            protected.signer_public_key,
        )

    with pytest.raises(InvalidUnlockKeyError):
        core_client.validate_unlock_key(
            protected.envelope,
            WalletUnlockKey(bytes(32)),
            protected.signer_public_key,
        )


def test_wallet_manager_session_uses_rust_signer_without_python_private_key(core_client):
    vector = _vector()
    manager = WalletManager(MemoryWalletStorage(), core_client=core_client)
    record = manager.import_secret("rust", vector["secret"], "passcode")
    session = manager.unlock(record.name, "passcode")

    assert record.signer_public_key == vector["public_key"]
    assert isinstance(session.wallet.signer, FresnicaProcessProtectedSigner)
    assert not hasattr(session.wallet.signer, "keypair")

    transaction = TransactionEnvelope.from_xdr(
        vector["unsigned_xdr_base64"],
        vector["network_passphrase"],
    )
    session.wallet.sign(transaction)
    assert transaction.to_xdr() == vector["signed_xdr_base64"]


def test_wallet_manager_upgrades_and_downgrades_same_account(core_client):
    vector = _vector()
    manager = WalletManager(MemoryWalletStorage(), core_client=core_client)
    watched = manager.add_watch("upgrade", vector["public_key"], network="testnet")

    upgraded = manager.upgrade_watch_with_secret("upgrade", vector["secret"], "passcode")
    assert upgraded.address == watched.address
    assert upgraded.signer_public_key == watched.address
    assert not upgraded.watch_only

    downgraded = manager.downgrade_to_watch("upgrade")
    assert downgraded.address == watched.address
    assert downgraded.watch_only
    assert downgraded.secret is None
    assert downgraded.signer_public_key is None


def test_verified_unlock_key_can_reopen_and_sign_without_passcode(core_client):
    vector = _vector()
    manager = WalletManager(MemoryWalletStorage(), core_client=core_client)
    record = manager.import_secret("system-path", vector["secret"], "passcode")
    unlock_key = manager.derive_verified_unlock_key(record.name, "passcode")

    manager.lock()
    session = manager.unlock_with_key(record.name, unlock_key)
    assert isinstance(session.wallet.signer, FresnicaProcessProtectedSigner)

    transaction = TransactionEnvelope.from_xdr(
        vector["unsigned_xdr_base64"],
        vector["network_passphrase"],
    )
    session.wallet.sign(transaction)
    assert transaction.to_xdr() == vector["signed_xdr_base64"]


def test_encrypted_backup_restores_with_passcode_only(core_client, tmp_path):
    vector = _vector()
    original = WalletManager(MemoryWalletStorage(), core_client=core_client)
    record = original.import_secret("backup", vector["secret"], "passcode")
    backup_path = tmp_path / "wallet-backup.json"
    original.backup(record.name, backup_path)

    restored = WalletManager(MemoryWalletStorage(), core_client=core_client)
    restored_record = restored.restore_backup(backup_path, make_default=True)
    session = restored.unlock(restored_record.name, "passcode")

    assert restored_record.address == vector["public_key"]
    assert restored_record.signer_public_key == vector["public_key"]
    assert isinstance(session.wallet.signer, FresnicaProcessProtectedSigner)
    assert not hasattr(session.wallet.signer, "keypair")


def test_generated_mnemonic_is_rust_owned_and_revealable(core_client):
    manager = WalletManager(MemoryWalletStorage(), core_client=core_client)
    record, mnemonic = manager.create_mnemonic(
        "generated",
        "passcode",
        strength=128,
        language="english",
    )

    assert len(mnemonic.split()) == 12
    assert record.signer_public_key == record.address
    revealed = manager.export_signing_material(record.name, "passcode")
    assert revealed["kind"] == "mnemonic"
    assert revealed["mnemonic"] == mnemonic
    assert revealed["index"] == 0
    assert revealed["language"] == "english"

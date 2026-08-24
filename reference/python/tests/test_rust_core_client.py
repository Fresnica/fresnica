import json
import os
from pathlib import Path

import pytest
from stellar_sdk import TransactionEnvelope

from fresnica.errors import InvalidPasswordError, InvalidUnlockKeyError
from fresnica.manager import WalletManager
from fresnica.rust_core_client import RustCoreClient
from fresnica.secret_store import WalletUnlockKey
from fresnica.signer import RustCoreProtectedSigner
from fresnica.storage import MemoryWalletStorage


VECTOR_PATH = (
    Path(__file__).parents[3]
    / "spec"
    / "test-vectors"
    / "transaction-signing-v1.json"
)


@pytest.fixture(scope="module")
def core_client():
    binary = os.environ.get("FRESNICA_CORE_BIN")
    if not binary:
        pytest.skip("Rust Core integration binary is not configured")
    return RustCoreClient(binary)


def _vector():
    return json.loads(VECTOR_PATH.read_text(encoding="utf-8"))["cases"][0]


def test_bridge_version(core_client):
    version = core_client.version()
    assert version["protocol_version"] == 1
    assert version["core_version"] == "0.1.0"


def test_secret_roundtrip_signs_exact_shared_vector(core_client):
    vector = _vector()
    protected = core_client.protect_secret(vector["secret"], "passcode")

    assert protected.public_key == vector["public_key"]
    assert vector["secret"] not in json.dumps(protected.envelope)

    unlock_key = core_client.derive_verified_unlock_key(
        protected.envelope,
        "passcode",
        protected.public_key,
    )
    core_client.validate_unlock_key(
        protected.envelope,
        unlock_key,
        protected.public_key,
    )
    signed_xdr = core_client.sign_transaction(
        protected.envelope,
        unlock_key,
        protected.public_key,
        vector["unsigned_xdr_base64"],
        vector["network_passphrase"],
    )
    assert signed_xdr == vector["signed_xdr_base64"]

    revealed = core_client.reveal(
        protected.envelope,
        "passcode",
        protected.public_key,
    )
    assert revealed == {"kind": "secret", "secret": vector["secret"]}


def test_wrong_passcode_and_unlock_key_fail_closed(core_client):
    vector = _vector()
    protected = core_client.protect_secret(vector["secret"], "correct")

    with pytest.raises(InvalidPasswordError):
        core_client.derive_verified_unlock_key(
            protected.envelope,
            "wrong",
            protected.public_key,
        )

    with pytest.raises(InvalidUnlockKeyError):
        core_client.validate_unlock_key(
            protected.envelope,
            WalletUnlockKey(bytes(32)),
            protected.public_key,
        )


def test_wallet_manager_session_uses_rust_signer_without_python_private_key(core_client):
    vector = _vector()
    manager = WalletManager(MemoryWalletStorage(), core_client=core_client)
    record = manager.import_secret("rust", vector["secret"], "passcode")
    session = manager.unlock(record.name, "passcode")

    assert isinstance(session.wallet.signer, RustCoreProtectedSigner)
    assert not hasattr(session.wallet.signer, "keypair")

    transaction = TransactionEnvelope.from_xdr(
        vector["unsigned_xdr_base64"],
        vector["network_passphrase"],
    )
    session.wallet.sign(transaction)
    assert transaction.to_xdr() == vector["signed_xdr_base64"]


def test_generated_mnemonic_is_rust_owned_and_revealable(core_client):
    manager = WalletManager(MemoryWalletStorage(), core_client=core_client)
    record, mnemonic = manager.create_mnemonic(
        "generated",
        "passcode",
        strength=128,
        language="english",
    )

    assert len(mnemonic.split()) == 12
    revealed = manager.export_signing_material(record.name, "passcode")
    assert revealed["kind"] == "mnemonic"
    assert revealed["mnemonic"] == mnemonic
    assert revealed["index"] == 0
    assert revealed["language"] == "english"

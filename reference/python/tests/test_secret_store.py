import pytest

from fresnica.errors import InvalidPasswordError
from fresnica.secret_store import (
    ARGON2_ITERATIONS,
    ARGON2_MEMORY_KIB,
    ARGON2_PARALLELISM,
    decrypt_secret,
    encrypt_secret,
)


def test_secret_round_trip_supports_unicode():
    payload = {"kind": "mnemonic", "mnemonic": "的 一 是 在", "passphrase": "密码"}
    envelope = encrypt_secret(payload, "wallet-password")

    assert envelope["ciphertext"]
    assert "的" not in str(envelope)
    assert decrypt_secret(envelope, "wallet-password") == payload


def test_wrong_wallet_password_fails_authentication():
    envelope = encrypt_secret({"kind": "secret", "secret": "S..."}, "correct")

    with pytest.raises(InvalidPasswordError):
        decrypt_secret(envelope, "wrong")


def test_new_password_envelope_uses_argon2id_v2():
    envelope = encrypt_secret({"kind": "secret", "secret": "S..."}, "correct")
    assert envelope["version"] == 2
    assert envelope["kdf"]["name"] == "argon2id"
    assert envelope["kdf"]["memory_kib"] == ARGON2_MEMORY_KIB
    assert envelope["kdf"]["iterations"] == ARGON2_ITERATIONS
    assert envelope["kdf"]["parallelism"] == ARGON2_PARALLELISM

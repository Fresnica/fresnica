import pytest

from fresnica.errors import InvalidPasswordError
from fresnica.secret_store import decrypt_secret, encrypt_secret


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

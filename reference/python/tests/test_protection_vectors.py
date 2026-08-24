import json
from pathlib import Path

import pytest

from fresnica.errors import InvalidPasswordError
from fresnica.secret_store import (
    decrypt_secret,
    decrypt_secret_with_key,
    decrypt_secret_with_unlock_key,
    derive_unlock_key,
)


VECTOR_PATH = Path(__file__).parents[3] / "spec" / "test-vectors" / "protection-v1.json"


def test_shared_protection_vectors_match_python_reference():
    vectors = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))

    assert vectors["schema"] == "fresnica-protection-v1"
    password_envelope = vectors["password"]["envelope"]
    password = vectors["password"]["password"]
    assert decrypt_secret(password_envelope, password) == vectors["payload"]
    with pytest.raises(InvalidPasswordError):
        decrypt_secret(password_envelope, "wrong")

    unlock_key = derive_unlock_key(password_envelope, password)
    assert decrypt_secret_with_unlock_key(password_envelope, unlock_key) == vectors["payload"]

    # Historical low-level key-AEAD vector. This is not the product system-auth path.
    key = bytes.fromhex(vectors["system"]["key_hex"])
    assert decrypt_secret_with_key(vectors["system"]["envelope"], key) == vectors["payload"]

import json
from pathlib import Path

import pytest

from fresnica.errors import InvalidPasswordError
from fresnica.secret_store import decrypt_secret, decrypt_secret_with_key


VECTOR_PATH = Path(__file__).parents[3] / "spec" / "test-vectors" / "protection-v1.json"


def test_shared_protection_vectors_match_python_reference():
    vectors = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))

    assert vectors["schema"] == "fresnica-protection-v1"
    assert (
        decrypt_secret(vectors["password"]["envelope"], vectors["password"]["password"])
        == vectors["payload"]
    )
    with pytest.raises(InvalidPasswordError):
        decrypt_secret(vectors["password"]["envelope"], "wrong")

    key = bytes.fromhex(vectors["system"]["key_hex"])
    assert decrypt_secret_with_key(vectors["system"]["envelope"], key) == vectors["payload"]

import hashlib
import json
from pathlib import Path

from stellar_sdk import Keypair


VECTORS = json.loads(
    (
        Path(__file__).resolve().parents[3]
        / "spec/test-vectors/message-signing-v1.json"
    ).read_text(encoding="utf-8")
)


def test_sep53_vectors_match_reference_python_semantics():
    prefix = VECTORS["prefix_utf8"].encode("utf-8")

    for case in VECTORS["cases"]:
        message = bytes.fromhex(case["message_hex"])
        encoded = prefix + message
        digest = hashlib.sha256(encoded).digest()
        expected_signature = bytes.fromhex(case["signature_hex"])

        assert encoded.hex() == case["encoded_message_hex"], case["name"]
        assert digest.hex() == case["message_hash_hex"], case["name"]

        signer = Keypair.from_secret(case["secret"])
        assert signer.public_key == case["public_key"], case["name"]
        assert signer.sign(digest) == expected_signature, case["name"]
        signer.verify(digest, expected_signature)


def test_sep53_does_not_sign_raw_message_bytes():
    case = VECTORS["cases"][0]
    signer = Keypair.from_secret(case["secret"])
    raw_message = bytes.fromhex(case["message_hex"])
    expected_signature = bytes.fromhex(case["signature_hex"])

    assert signer.sign(hashlib.sha256(raw_message).digest()) != expected_signature

import json
from pathlib import Path

from stellar_sdk import TransactionEnvelope


VECTOR_PATH = Path(__file__).parents[3] / "spec" / "test-vectors" / "transaction-signing-v1.json"


def test_shared_transaction_signing_vectors_match_python_reference():
    vectors = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))

    assert vectors["schema"] == "fresnica-transaction-signing-v1"
    assert vectors["cases"]

    for vector in vectors["cases"]:
        envelope = TransactionEnvelope.from_xdr(
            vector["unsigned_xdr_base64"],
            vector["network_passphrase"],
        )
        assert envelope.hash_hex() == vector["transaction_hash_hex"]

        envelope.sign(vector["secret"])
        assert len(envelope.signatures) == 1
        assert envelope.signatures[0].signature.hex() == vector["signature_hex"]
        assert envelope.signatures[0].signature_hint.hex() == vector["signature_hint_hex"]
        assert envelope.to_xdr() == vector["signed_xdr_base64"]

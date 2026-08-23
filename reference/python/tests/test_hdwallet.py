"""Cross-language SEP-0005 wallet derivation conformance."""

import json
from pathlib import Path

from fresnica.hdwallet import derive_account, detect_mnemonic_language


VECTORS = json.loads(
    (Path(__file__).resolve().parents[3] / "spec/test-vectors/wallet-v1.json").read_text(
        encoding="utf-8"
    )
)


def test_sep0005_wallet_derivation_vectors():
    for case in VECTORS["derivation"]:
        keypair = derive_account(
            case["mnemonic"],
            passphrase=case["passphrase"],
            index=int(case["index"]),
            language=case["language"],
        )

        assert keypair.public_key == case["expected_public_key"], case["name"]


def test_vector_mnemonics_are_detected_as_the_declared_language():
    for case in VECTORS["derivation"]:
        detected = detect_mnemonic_language(case["mnemonic"])
        assert detected.value == case["language"], case["name"]

"""BIP39 mnemonic handling.

Reference implementation.

Supports:
- English mnemonic
- Simplified/Traditional Chinese mnemonic
- BIP39 passphrase

The implementation intentionally keeps only the seed derivation layer.
HD derivation is handled by hdwallet.py.
"""

import unicodedata
import hashlib


def normalize_text(value: str) -> str:
    """Apply BIP39 NFKD normalization."""
    return unicodedata.normalize("NFKD", value)


def mnemonic_to_seed(mnemonic: str, passphrase: str = "") -> bytes:
    """Convert BIP39 mnemonic into a 64-byte seed.

    BIP39 specifies:

        PBKDF2-HMAC-SHA512
        password = normalized mnemonic
        salt = "mnemonic" + normalized passphrase
        iterations = 2048

    """
    mnemonic = normalize_text(mnemonic)
    passphrase = normalize_text(passphrase)

    salt = "mnemonic" + passphrase

    return hashlib.pbkdf2_hmac(
        "sha512",
        mnemonic.encode("utf-8"),
        salt.encode("utf-8"),
        2048,
        dklen=64,
    )


def validate_mnemonic(mnemonic: str) -> bool:
    """Placeholder for full BIP39 checksum validation."""
    return bool(normalize_text(mnemonic).strip())

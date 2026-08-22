"""BIP39 mnemonic handling.

Fresnica wallet abstraction layer.

The implementation intentionally delegates BIP39 cryptography to a
well-tested external library. Fresnica defines the wallet interface,
not a replacement for the BIP39 standard.
"""

from mnemonic import Mnemonic


def mnemonic_to_seed(mnemonic: str, passphrase: str = "") -> bytes:
    """Convert BIP39 mnemonic into seed.

    Supports BIP39 languages provided by the library, including Chinese.
    """
    words = mnemonic.strip()

    # Language detection allows English and Chinese mnemonics.
    # BIP39 checksum validation is delegated to the library.
    for language in (
        "english",
        "chinese_simplified",
        "chinese_traditional",
        "japanese",
        "korean",
        "spanish",
        "french",
        "italian",
    ):
        try:
            return Mnemonic(language).to_seed(words, passphrase)
        except Exception:
            continue

    raise ValueError("Invalid BIP39 mnemonic")


def validate_mnemonic(mnemonic: str) -> bool:
    """Validate BIP39 checksum."""
    words = mnemonic.strip()

    for language in (
        "english",
        "chinese_simplified",
        "chinese_traditional",
    ):
        if Mnemonic(language).check(words):
            return True

    return False

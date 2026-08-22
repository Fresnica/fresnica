"""Compatibility helpers for Stellar mnemonic phrases.

Fresnica delegates mnemonic validation, generation, and SEP-0005 derivation to
Stellar Python SDK instead of maintaining a second BIP39 implementation.
"""

from stellar_sdk.sep.mnemonic import Language, StellarMnemonic

from .hdwallet import (
    SUPPORTED_LANGUAGES,
    detect_mnemonic_language,
    generate_mnemonic_phrase,
    normalize_language,
)


def validate_mnemonic(mnemonic: str, language: Language | str | None = None) -> bool:
    words = mnemonic.strip()
    if language is not None:
        return StellarMnemonic(normalize_language(language)).check(words)
    try:
        detect_mnemonic_language(words)
    except ValueError:
        return False
    return True


__all__ = [
    "SUPPORTED_LANGUAGES",
    "detect_mnemonic_language",
    "generate_mnemonic_phrase",
    "normalize_language",
    "validate_mnemonic",
]

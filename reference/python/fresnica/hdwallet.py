"""Stellar mnemonic account derivation.

All BIP39 / SEP-0005 derivation is delegated to Stellar Python SDK.
"""

from stellar_sdk import Keypair
from stellar_sdk.sep.mnemonic import Language, StellarMnemonic


SUPPORTED_LANGUAGES = tuple(Language)


def normalize_language(language: Language | str) -> Language:
    if isinstance(language, Language):
        return language
    try:
        return Language(language)
    except ValueError as exc:
        raise ValueError(f"Unsupported mnemonic language: {language}") from exc


def detect_mnemonic_language(mnemonic: str) -> Language:
    words = mnemonic.strip()
    matches = [
        language
        for language in SUPPORTED_LANGUAGES
        if StellarMnemonic(language).check(words)
    ]
    if len(matches) == 1:
        return matches[0]
    if not matches:
        raise ValueError("Invalid or unsupported Stellar mnemonic phrase")
    raise ValueError("Mnemonic language is ambiguous; specify it explicitly")


def derive_account(
    mnemonic: str,
    passphrase: str = "",
    index: int = 0,
    language: Language | str | None = None,
) -> Keypair:
    """Derive a Stellar Keypair using the SDK's SEP-0005 implementation."""
    if language is None:
        language = detect_mnemonic_language(mnemonic)
    else:
        language = normalize_language(language)

    return Keypair.from_mnemonic_phrase(
        mnemonic.strip(),
        language=language,
        passphrase=passphrase,
        index=index,
    )


def generate_mnemonic_phrase(
    language: Language | str = Language.ENGLISH,
    strength: int = 256,
) -> str:
    return Keypair.generate_mnemonic_phrase(
        language=normalize_language(language),
        strength=strength,
    )

from stellar_sdk.sep.mnemonic import Language

from fresnica.hdwallet import normalize_language


def test_language_string_normalizes_to_sdk_enum():
    assert normalize_language("chinese_simplified") == Language("chinese_simplified")

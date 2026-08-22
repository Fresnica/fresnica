"""Mnemonic and Stellar account tests."""

from fresnica.hdwallet import derive_account


def test_english_mnemonic_derivation():
    mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    keypair = derive_account(mnemonic)

    assert keypair.public_key.startswith("G")


def test_passphrase_changes_account():
    mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

    a = derive_account(mnemonic)
    b = derive_account(mnemonic, passphrase="test")

    assert a.public_key != b.public_key

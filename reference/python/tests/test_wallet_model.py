"""Tests for Fresnica wallet model."""

from fresnica.wallet import Wallet


MNEMONIC = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"


def test_mnemonic_wallet_can_sign():
    wallet = Wallet.from_mnemonic(MNEMONIC)

    assert wallet.can_sign() is True
    assert wallet.address().startswith("G")


def test_watch_only_wallet_cannot_sign():
    wallet = Wallet.from_address("GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF")

    assert wallet.can_sign() is False

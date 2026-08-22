"""Wallet model tests."""

from fresnica.wallet import Wallet


# Test vectors will be replaced with fixed Stellar test vectors.
MNEMONIC = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"


def test_mnemonic_and_secret_share_identity():
    wallet = Wallet.from_mnemonic(MNEMONIC)

    secret_wallet = Wallet.from_secret(
        wallet.signer.keypair.secret
    )

    assert wallet.address() == secret_wallet.address()


def test_watch_only_wallet_cannot_sign():
    wallet = Wallet.from_address(
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    )

    assert wallet.can_sign() is False

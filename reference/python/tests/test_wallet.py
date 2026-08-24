"""Wallet model tests."""

import pytest
from stellar_sdk import StrKey

from fresnica.wallet import AccountKind, Wallet


# Test vectors will be replaced with fixed Stellar test vectors.
MNEMONIC = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"


def test_mnemonic_and_secret_share_identity():
    wallet = Wallet.from_mnemonic(MNEMONIC)

    secret_wallet = Wallet.from_secret(
        wallet.signer.keypair.secret
    )

    assert wallet.address() == secret_wallet.address()
    assert wallet.account().kind is AccountKind.CLASSIC
    assert wallet.account().public_key == wallet.address()


def test_watch_only_wallet_cannot_sign():
    wallet = Wallet.from_address(
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    )

    assert wallet.can_sign() is False
    assert wallet.account().is_classic


def test_contract_account_identity_is_representable_without_classic_public_key():
    address = StrKey.encode_contract(bytes(range(32)))

    wallet = Wallet.from_contract_address(address)

    assert wallet.address() == address
    assert wallet.account().kind is AccountKind.CONTRACT
    assert wallet.account().public_key is None
    assert wallet.account().index is None
    assert wallet.can_sign() is False


def test_contract_account_address_is_validated():
    with pytest.raises(ValueError, match="Invalid Stellar contract address"):
        Wallet.from_contract_address("C-not-a-contract-address")

"""Wallet model tests."""

import pytest
from stellar_sdk import Keypair, StrKey

from fresnica.signer import StellarKeypairSigner
from fresnica.wallet import AccountKind, Wallet


MNEMONIC = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"


def test_mnemonic_and_secret_share_identity():
    wallet = Wallet.from_mnemonic(MNEMONIC)

    secret_wallet = Wallet.from_secret(
        wallet.signer.keypair.secret
    )

    assert wallet.address() == secret_wallet.address()
    assert wallet.account().kind is AccountKind.CLASSIC
    assert wallet.account().public_key == wallet.address()
    assert wallet.signer_public_key() == wallet.address()


def test_watch_only_wallet_cannot_sign():
    wallet = Wallet.from_address(
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    )

    assert wallet.can_sign() is False
    assert wallet.signer_public_key() is None
    assert wallet.account().is_classic


def test_classic_account_can_reference_a_different_local_signer():
    account = Keypair.random()
    signer_keypair = Keypair.random()
    wallet = Wallet.from_address(account.public_key).with_signer(
        StellarKeypairSigner(signer_keypair)
    )

    assert wallet.address() == account.public_key
    assert wallet.signer_public_key() == signer_keypair.public_key
    assert wallet.can_sign()


def test_contract_account_identity_is_representable_without_classic_public_key():
    address = StrKey.encode_contract(bytes(range(32)))

    wallet = Wallet.from_address(address)

    assert wallet.address() == address
    assert wallet.account().kind is AccountKind.CONTRACT
    assert wallet.account().public_key is None
    assert wallet.account().index is None
    assert wallet.can_sign() is False


def test_contract_account_address_is_validated():
    with pytest.raises(ValueError, match="Invalid Stellar contract address"):
        Wallet.from_contract_address("C-not-a-contract-address")


def test_generic_account_parser_rejects_unsupported_input():
    with pytest.raises(ValueError, match="Invalid or unsupported Stellar account address"):
        Wallet.from_address("not-an-address")

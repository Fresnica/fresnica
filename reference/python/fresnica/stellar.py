"""Stellar helpers.

Delegates protocol details to stellar-sdk.
"""

from stellar_sdk import Keypair


def encode_address(public_key: bytes) -> str:
    """Encode a raw public key into Stellar G address."""
    return Keypair.from_public_key(public_key).public_key


def address_from_keypair(keypair: Keypair) -> str:
    """Return Stellar account address."""
    return keypair.public_key

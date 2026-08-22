"""Small Stellar SDK compatibility helpers.

Protocol behavior stays in ``stellar-sdk``. These helpers only keep Fresnica
call sites explicit and testable.
"""

from stellar_sdk import Keypair


def keypair_from_address(address: str) -> Keypair:
    return Keypair.from_public_key(address)


def address_from_keypair(keypair: Keypair) -> str:
    return keypair.public_key

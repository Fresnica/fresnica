"""Fresnica signer abstraction.

A signer proves ownership of an account.
The account identity is separated from the signing mechanism.
"""

from stellar_sdk import Keypair


class Signer:
    """Base signer interface."""

    @property
    def public_key(self) -> str:
        raise NotImplementedError

    def sign(self, transaction):
        raise NotImplementedError


class StellarKeypairSigner(Signer):
    """Software signer backed by Stellar SDK Keypair."""

    def __init__(self, keypair: Keypair):
        self.keypair = keypair

    @property
    def public_key(self) -> str:
        return self.keypair.public_key

    def sign(self, transaction):
        transaction.sign(self.keypair)
        return transaction

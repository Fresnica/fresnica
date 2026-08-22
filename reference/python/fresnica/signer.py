"""Fresnica signer abstraction."""

from abc import ABC, abstractmethod

from stellar_sdk import Keypair


class Signer(ABC):
    @property
    @abstractmethod
    def public_key(self) -> str:
        raise NotImplementedError

    @abstractmethod
    def sign(self, transaction):
        raise NotImplementedError


class StellarKeypairSigner(Signer):
    """Software signer backed by Stellar SDK Keypair."""

    def __init__(self, keypair: Keypair):
        if not keypair.can_sign():
            raise ValueError("Keypair does not contain signing material")
        self.keypair = keypair

    @property
    def public_key(self) -> str:
        return self.keypair.public_key

    def sign(self, transaction):
        transaction.sign(self.keypair)
        return transaction

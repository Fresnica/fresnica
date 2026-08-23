"""Fresnica signer abstraction."""

from abc import ABC, abstractmethod
from collections.abc import Callable

from stellar_sdk import Keypair
from stellar_sdk.decorated_signature import DecoratedSignature
from stellar_sdk.exceptions import BadSignatureError

from .errors import SignerError


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


class ExternalEd25519Signer(Signer):
    """Signer adapter for a device/process that signs a Stellar transaction hash.

    The provider receives exactly the 32-byte hash returned by the Stellar SDK
    envelope. Fresnica verifies the returned Ed25519 signature against the
    declared public key before adding a decorated signature to the envelope.
    No private signing material is stored by this adapter.
    """

    def __init__(self, public_key: str, sign_hash: Callable[[bytes], bytes]):
        self.keypair = Keypair.from_public_key(public_key)
        if not callable(sign_hash):
            raise TypeError("External sign_hash provider must be callable")
        self.sign_hash = sign_hash

    @property
    def public_key(self) -> str:
        return self.keypair.public_key

    def sign(self, transaction):
        tx_hash = transaction.hash()
        try:
            signature = self.sign_hash(tx_hash)
        except Exception as exc:
            raise SignerError("External signer failed") from exc
        if not isinstance(signature, bytes) or len(signature) != 64:
            raise SignerError("External signer must return a 64-byte Ed25519 signature")
        try:
            self.keypair.verify(tx_hash, signature)
        except BadSignatureError as exc:
            raise SignerError("External signer returned a signature for the wrong key or payload") from exc

        decorated = DecoratedSignature(self.keypair.signature_hint(), signature)
        if decorated in transaction.signatures:
            raise SignerError("External signer has already signed this transaction")
        transaction.signatures.append(decorated)
        return transaction

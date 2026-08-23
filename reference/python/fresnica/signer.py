"""Fresnica signer abstraction."""

from abc import ABC, abstractmethod
from collections.abc import Callable
from dataclasses import dataclass

from stellar_sdk import Keypair
from stellar_sdk.decorated_signature import DecoratedSignature
from stellar_sdk.exceptions import BadSignatureError

from .errors import SignerError


@dataclass(frozen=True)
class ExternalSigningRequest:
    """Public transaction material an external signer may inspect before signing."""

    transaction_hash: bytes
    transaction_xdr: str
    network_passphrase: str


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
    """Verified adapter for a hardware/device/process Ed25519 signer.

    The provider receives public transaction material including XDR and the
    exact 32-byte hash that Stellar validators expect to be signed. Fresnica
    verifies the returned signature against the declared public key before
    mutating the transaction envelope. No private signing material is stored.
    """

    def __init__(
        self,
        public_key: str,
        sign_request: Callable[[ExternalSigningRequest], bytes],
    ):
        self.keypair = Keypair.from_public_key(public_key)
        if not callable(sign_request):
            raise TypeError("External sign_request provider must be callable")
        self.sign_request = sign_request

    @property
    def public_key(self) -> str:
        return self.keypair.public_key

    def sign(self, transaction):
        tx_hash = transaction.hash()
        request = ExternalSigningRequest(
            transaction_hash=tx_hash,
            transaction_xdr=transaction.to_xdr(),
            network_passphrase=transaction.network_passphrase,
        )
        try:
            signature = self.sign_request(request)
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

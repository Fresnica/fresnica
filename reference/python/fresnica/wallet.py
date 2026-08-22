"""Fresnica wallet model.

Wallet manages accounts and signers.

Account represents identity.
Signer represents ownership proof.
"""

from dataclasses import dataclass

from stellar_sdk import Keypair

from .hdwallet import derive_account
from .signer import StellarKeypairSigner


@dataclass
class Account:
    """A Stellar account identity."""

    index: int
    address: str
    public_key: str


class Wallet:
    """Stellar wallet abstraction."""

    def __init__(self, signer: StellarKeypairSigner):
        self.signer = signer
        self._account = Account(
            index=0,
            address=signer.public_key,
            public_key=signer.public_key,
        )

    @classmethod
    def from_mnemonic(
        cls,
        mnemonic: str,
        passphrase: str = "",
        index: int = 0,
    ):
        keypair = derive_account(
            mnemonic,
            passphrase,
            index,
        )
        return cls(StellarKeypairSigner(keypair))

    @classmethod
    def from_secret(cls, secret: str):
        keypair = Keypair.from_secret(secret)
        return cls(StellarKeypairSigner(keypair))

    def account(self) -> Account:
        return self._account

    def address(self) -> str:
        return self._account.address

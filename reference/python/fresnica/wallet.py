"""Fresnica wallet model.

Wallet manages accounts and optional signers.

Account represents identity.
Signer represents ownership proof.
A wallet may exist without a signer as a watch-only wallet.
"""

from dataclasses import dataclass
from typing import Optional

from stellar_sdk import Keypair

from .hdwallet import derive_account
from .signer import Signer, StellarKeypairSigner


@dataclass
class Account:
    """A Stellar account identity."""

    index: int
    address: str
    public_key: str


class Wallet:
    """Stellar wallet abstraction.

    A wallet can be:
    - signing wallet
    - watch-only wallet
    """

    def __init__(self, account: Account, signer: Optional[Signer] = None):
        self._account = account
        self.signer = signer

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
        return cls.from_keypair(keypair, index)

    @classmethod
    def from_secret(cls, secret: str):
        keypair = Keypair.from_secret(secret)
        return cls.from_keypair(keypair)

    @classmethod
    def from_address(cls, address: str):
        """Create a watch-only wallet."""
        account = Account(
            index=0,
            address=address,
            public_key=address,
        )
        return cls(account)

    @classmethod
    def from_keypair(cls, keypair: Keypair, index: int = 0):
        signer = StellarKeypairSigner(keypair)
        account = Account(
            index=index,
            address=signer.public_key,
            public_key=signer.public_key,
        )
        return cls(account, signer)

    def account(self) -> Account:
        return self._account

    def address(self) -> str:
        return self._account.address

    def can_sign(self) -> bool:
        return self.signer is not None

    def sign(self, transaction):
        if not self.can_sign():
            raise RuntimeError("Watch-only wallet cannot sign")
        return self.signer.sign(transaction)

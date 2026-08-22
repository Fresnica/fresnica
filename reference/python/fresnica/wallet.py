"""Fresnica wallet model.

Wallet manages accounts and optional signers.

Account represents identity.
Signer represents ownership proof.
A wallet without a signer is watch-only.
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

    def __init__(self, account: Account, signer=None):
        self._account = account
        self.signer = signer

    @classmethod
    def from_mnemonic(
        cls,
        mnemonic: str,
        passphrase: str = "",
        index: int = 0,
    ):
        keypair = derive_account(mnemonic, passphrase, index)
        return cls(
            Account(index, keypair.public_key, keypair.public_key),
            StellarKeypairSigner(keypair),
        )

    @classmethod
    def from_secret(cls, secret: str):
        keypair = Keypair.from_secret(secret)
        return cls(
            Account(0, keypair.public_key, keypair.public_key),
            StellarKeypairSigner(keypair),
        )

    @classmethod
    def from_address(cls, address: str):
        return cls(
            Account(0, address, address),
            None,
        )

    def account(self) -> Account:
        return self._account

    def address(self) -> str:
        return self._account.address

    def can_sign(self) -> bool:
        return self.signer is not None

    def sign(self, transaction):
        if not self.signer:
            raise RuntimeError("Watch-only wallet cannot sign")
        return self.signer.sign(transaction)

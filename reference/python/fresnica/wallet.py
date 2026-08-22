"""Fresnica wallet model.

A wallet is a higher-level abstraction above Stellar SDK Keypair.

Stellar terminology is slightly different from many chains:
- account
- address
- public key

all refer to the same identity in normal wallet usage.
Fresnica keeps a clear API while preserving Stellar semantics.
"""

from dataclasses import dataclass

from .hdwallet import derive_account


@dataclass
class Account:
    """A Stellar account representation."""

    index: int
    address: str
    public_key: str


class Wallet:
    """Mnemonic based Stellar wallet."""

    def __init__(self, mnemonic: str, passphrase: str = ""):
        self.mnemonic = mnemonic
        self.passphrase = passphrase
        self._accounts = {}

    def account(self, index: int = 0) -> Account:
        """Get or derive an account."""
        if index not in self._accounts:
            keypair = derive_account(
                self.mnemonic,
                self.passphrase,
                index,
            )

            self._accounts[index] = Account(
                index=index,
                address=keypair.public_key,
                public_key=keypair.public_key,
            )

        return self._accounts[index]

    def address(self, index: int = 0) -> str:
        return self.account(index).address

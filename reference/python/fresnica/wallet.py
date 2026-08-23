"""Fresnica wallet identity model.

Wallet represents an account plus optional signing capability. Network state,
balances, transaction history, and persistence live outside this object.
"""

from dataclasses import dataclass

from stellar_sdk import Keypair

from .errors import WatchOnlyError
from .hdwallet import derive_account
from .signer import Signer, StellarKeypairSigner


@dataclass(frozen=True)
class Account:
    index: int
    address: str
    public_key: str


class Wallet:
    def __init__(self, account: Account, signer: Signer | None = None):
        if signer is not None and signer.public_key != account.public_key:
            raise ValueError("Signer public key does not match wallet account")
        self._account = account
        self.signer = signer

    @classmethod
    def from_mnemonic(
        cls,
        mnemonic: str,
        passphrase: str = "",
        index: int = 0,
        language=None,
    ) -> "Wallet":
        keypair = derive_account(
            mnemonic,
            passphrase=passphrase,
            index=index,
            language=language,
        )
        account = Account(index, keypair.public_key, keypair.public_key)
        return cls(account, StellarKeypairSigner(keypair))

    @classmethod
    def from_secret(cls, secret: str) -> "Wallet":
        keypair = Keypair.from_secret(secret.strip())
        account = Account(0, keypair.public_key, keypair.public_key)
        return cls(account, StellarKeypairSigner(keypair))

    @classmethod
    def from_address(cls, address: str) -> "Wallet":
        # Let the Stellar SDK validate the public key instead of duplicating StrKey logic.
        keypair = Keypair.from_public_key(address.strip())
        account = Account(0, keypair.public_key, keypair.public_key)
        return cls(account)

    @classmethod
    def from_signer(cls, signer: Signer, index: int = 0) -> "Wallet":
        """Create a signing wallet whose key material lives behind a Signer."""
        keypair = Keypair.from_public_key(signer.public_key)
        account = Account(index, keypair.public_key, keypair.public_key)
        return cls(account, signer)

    from_public_key = from_address

    def account(self) -> Account:
        return self._account

    def address(self) -> str:
        return self._account.address

    def can_sign(self) -> bool:
        return self.signer is not None

    def sign(self, transaction):
        if self.signer is None:
            raise WatchOnlyError("Watch-only wallet cannot sign transactions")
        return self.signer.sign(transaction)

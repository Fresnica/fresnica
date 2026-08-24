"""Fresnica wallet identity model.

Wallet represents an account plus optional signing capability. Network state,
balances, transaction history, and persistence live outside this object.
"""

from dataclasses import dataclass
from enum import Enum

from stellar_sdk import Keypair, StrKey

from .errors import WatchOnlyError
from .hdwallet import derive_account
from .signer import Signer, StellarKeypairSigner


class AccountKind(str, Enum):
    CLASSIC = "classic"
    CONTRACT = "contract"


@dataclass(frozen=True)
class Account:
    index: int | None
    address: str
    public_key: str | None
    kind: AccountKind = AccountKind.CLASSIC

    @classmethod
    def classic(cls, public_key: str, index: int = 0) -> "Account":
        keypair = Keypair.from_public_key(public_key.strip())
        return cls(index, keypair.public_key, keypair.public_key, AccountKind.CLASSIC)

    @classmethod
    def contract(cls, address: str) -> "Account":
        address = address.strip()
        if not StrKey.is_valid_contract(address):
            raise ValueError("Invalid Stellar contract address")
        return cls(None, address, None, AccountKind.CONTRACT)

    @property
    def is_classic(self) -> bool:
        return self.kind is AccountKind.CLASSIC

    @property
    def is_contract(self) -> bool:
        return self.kind is AccountKind.CONTRACT


class Wallet:
    def __init__(self, account: Account, signer: Signer | None = None):
        if account.is_classic:
            if account.public_key is None:
                raise ValueError("Classic account requires an Ed25519 public key")
            if signer is not None and signer.public_key != account.public_key:
                raise ValueError("Signer public key does not match wallet account")
        elif signer is not None:
            raise ValueError("Contract account signer support is not implemented")
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
        account = Account.classic(keypair.public_key, index=index)
        return cls(account, StellarKeypairSigner(keypair))

    @classmethod
    def from_secret(cls, secret: str) -> "Wallet":
        keypair = Keypair.from_secret(secret.strip())
        account = Account.classic(keypair.public_key)
        return cls(account, StellarKeypairSigner(keypair))

    @classmethod
    def from_address(cls, address: str) -> "Wallet":
        """Create a watch-only classic-account wallet from a G address."""
        return cls(Account.classic(address))

    @classmethod
    def from_contract_address(cls, address: str) -> "Wallet":
        """Represent a contract-account identity without implementing its runtime."""
        return cls(Account.contract(address))

    @classmethod
    def from_signer(cls, signer: Signer, index: int = 0) -> "Wallet":
        """Create a signing classic wallet whose key material lives behind a Signer."""
        account = Account.classic(signer.public_key, index=index)
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
            raise WatchOnlyError("Wallet has no signer for this account")
        return self.signer.sign(transaction)

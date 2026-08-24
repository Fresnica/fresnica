"""Fresnica wallet identity model.

Wallet is a client-side aggregate of an account identity plus an optional local
signer capability. Account identity and signer identity are deliberately
separate: a Stellar account may be watch-only or may authorize additional /
multisig signer keys that differ from the account address.

Network state, balances, authorization lookup, transaction history, and
persistence live outside this object.
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

    @classmethod
    def parse(cls, address: str, index: int = 0) -> "Account":
        text = address.strip()
        try:
            return cls.classic(text, index=index)
        except (TypeError, ValueError):
            if StrKey.is_valid_contract(text):
                return cls.contract(text)
            raise ValueError("Invalid or unsupported Stellar account address")

    @property
    def is_classic(self) -> bool:
        return self.kind is AccountKind.CLASSIC

    @property
    def is_contract(self) -> bool:
        return self.kind is AccountKind.CONTRACT


class Wallet:
    def __init__(self, account: Account, signer: Signer | None = None):
        if account.is_classic and account.public_key is None:
            raise ValueError("Classic account requires an Ed25519 public key")
        if account.is_contract and signer is not None:
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
        """Create a watch-only wallet from a supported Stellar account identity."""
        return cls(Account.parse(address))

    @classmethod
    def from_contract_address(cls, address: str) -> "Wallet":
        """Represent a contract identity without implementing its authorization runtime."""
        return cls(Account.contract(address))

    @classmethod
    def from_signer(cls, signer: Signer, index: int = 0) -> "Wallet":
        """Create the simple master-key case where account and signer are identical."""
        account = Account.classic(signer.public_key, index=index)
        return cls(account, signer)

    from_public_key = from_address

    def with_signer(self, signer: Signer) -> "Wallet":
        """Attach a local signer capability without changing account identity.

        Whether this signer is currently authorized for the account is a
        ledger/client-policy question. Core/signers remain responsible for
        proving signer identity and signature validity.
        """
        return Wallet(self._account, signer)

    def account(self) -> Account:
        return self._account

    def address(self) -> str:
        return self._account.address

    def signer_public_key(self) -> str | None:
        return self.signer.public_key if self.signer is not None else None

    def can_sign(self) -> bool:
        return self.signer is not None

    def sign(self, transaction):
        if self.signer is None:
            raise WatchOnlyError("Wallet has no local signer capability")
        return self.signer.sign(transaction)

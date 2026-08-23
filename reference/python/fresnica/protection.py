"""Protection providers for local wallet signing material.

Protection answers how Fresnica may recover locally stored signing material.
It is deliberately separate from signing itself: hardware, remote, and future
contract-account signers do not need to expose local secret material here.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
import os

from .errors import ProtectionError, ProtectionUnavailableError, WalletError
from .secret_store import (
    decrypt_secret,
    decrypt_secret_with_key,
    encrypt_secret,
    encrypt_secret_with_key,
)


PROTECTED_SECRET_FORMAT = "fresnica-protected-secret"
PROTECTED_SECRET_VERSION = 1


@dataclass(frozen=True)
class ProtectionCredential:
    kind: str
    value: object | None = None

    @classmethod
    def password(cls, password: str) -> "ProtectionCredential":
        return cls("password", password)

    @classmethod
    def system(cls) -> "ProtectionCredential":
        return cls("system")


class ProtectionProvider(ABC):
    kind: str

    @abstractmethod
    def protect(self, payload: dict, credential: ProtectionCredential) -> dict:
        raise NotImplementedError

    @abstractmethod
    def unprotect(self, envelope: dict, credential: ProtectionCredential) -> dict:
        raise NotImplementedError

    def _require_kind(self, credential: ProtectionCredential) -> None:
        if credential.kind != self.kind:
            raise ProtectionError(
                f"Protection credential {credential.kind!r} cannot unlock {self.kind!r} material"
            )


class PasswordProtectionProvider(ProtectionProvider):
    kind = "password"

    def protect(self, payload: dict, credential: ProtectionCredential) -> dict:
        self._require_kind(credential)
        if not isinstance(credential.value, str):
            raise ProtectionError("Password protection requires a password credential")
        return encrypt_secret(payload, credential.value)

    def unprotect(self, envelope: dict, credential: ProtectionCredential) -> dict:
        self._require_kind(credential)
        if not isinstance(credential.value, str):
            raise ProtectionError("Password protection requires a password credential")
        return decrypt_secret(envelope, credential.value)


class SystemKeyStore(ABC):
    """Platform boundary for OS-protected wrapping keys.

    A desktop/mobile implementation may map this to Keychain, DPAPI/Hello,
    Android Keystore, or another platform facility. Loading a key may itself
    trigger system authentication. The Python reference intentionally does not
    choose one platform backend.
    """

    @abstractmethod
    def store_key(self, key: bytes) -> str:
        raise NotImplementedError

    @abstractmethod
    def load_key(self, reference: str) -> bytes:
        raise NotImplementedError


class SystemProtectionProvider(ProtectionProvider):
    kind = "system"

    def __init__(self, key_store: SystemKeyStore):
        self.key_store = key_store

    def protect(self, payload: dict, credential: ProtectionCredential) -> dict:
        self._require_kind(credential)
        key = os.urandom(32)
        try:
            reference = self.key_store.store_key(key)
        except Exception as exc:
            raise ProtectionUnavailableError(
                "System protection could not store a wallet protection key"
            ) from exc
        if not isinstance(reference, str) or not reference:
            raise ProtectionUnavailableError(
                "System protection returned an invalid key reference"
            )
        return {
            "key_reference": reference,
            "secret": encrypt_secret_with_key(payload, key),
        }

    def unprotect(self, envelope: dict, credential: ProtectionCredential) -> dict:
        self._require_kind(credential)
        try:
            reference = envelope["key_reference"]
            secret = envelope["secret"]
            if not isinstance(reference, str) or not isinstance(secret, dict):
                raise TypeError
            key = self.key_store.load_key(reference)
        except ProtectionUnavailableError:
            raise
        except Exception as exc:
            raise ProtectionUnavailableError(
                "System protection could not access the wallet protection key"
            ) from exc
        return decrypt_secret_with_key(secret, key)


class ProtectionRegistry:
    def __init__(self, providers: list[ProtectionProvider] | None = None):
        self._providers: dict[str, ProtectionProvider] = {}
        self.register(PasswordProtectionProvider())
        for provider in providers or []:
            self.register(provider)

    def register(self, provider: ProtectionProvider) -> None:
        self._providers[provider.kind] = provider

    def kind_for(self, envelope: dict) -> str:
        if not isinstance(envelope, dict):
            raise WalletError("Wallet signing material is corrupted")
        if envelope.get("format") == PROTECTED_SECRET_FORMAT:
            if envelope.get("version") != PROTECTED_SECRET_VERSION:
                raise WalletError("Unsupported wallet protection format")
            protection = envelope.get("protection")
            if not isinstance(protection, dict) or not isinstance(
                protection.get("type"), str
            ):
                raise WalletError("Wallet protection metadata is corrupted")
            return protection["type"]
        # Pre-provider Fresnica records used the password envelope directly.
        if envelope.get("cipher") == "aes-256-gcm" and "kdf" in envelope:
            return "password"
        raise WalletError("Unsupported wallet protection format")

    def protect(self, payload: dict, credential: ProtectionCredential) -> dict:
        provider = self._provider(credential.kind)
        return self.wrap(credential.kind, provider.protect(payload, credential))

    def unprotect(self, envelope: dict, credential: ProtectionCredential) -> dict:
        kind = self.kind_for(envelope)
        if credential.kind != kind:
            raise ProtectionError(
                f"Wallet uses {kind!r} protection, not {credential.kind!r}"
            )
        provider = self._provider(kind)
        if self.is_legacy_password(envelope):
            return provider.unprotect(envelope, credential)
        payload = envelope.get("payload")
        if not isinstance(payload, dict):
            raise WalletError("Wallet protection payload is corrupted")
        return provider.unprotect(payload, credential)

    def wrap(self, kind: str, provider_envelope: dict) -> dict:
        self._provider(kind)
        return {
            "format": PROTECTED_SECRET_FORMAT,
            "version": PROTECTED_SECRET_VERSION,
            "protection": {"type": kind},
            "payload": provider_envelope,
        }

    def is_legacy_password(self, envelope: dict) -> bool:
        return (
            isinstance(envelope, dict)
            and envelope.get("format") != PROTECTED_SECRET_FORMAT
            and envelope.get("cipher") == "aes-256-gcm"
            and "kdf" in envelope
        )

    def migrate_legacy_password(self, envelope: dict) -> dict:
        if not self.is_legacy_password(envelope):
            return envelope
        return self.wrap("password", envelope)

    def _provider(self, kind: str) -> ProtectionProvider:
        try:
            return self._providers[kind]
        except KeyError as exc:
            raise ProtectionUnavailableError(
                f"Protection provider is not available: {kind}"
            ) from exc

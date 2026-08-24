"""Protection semantics for local wallet signing material.

The Python reference mirrors the production Client/Core boundary: password
protection defines the canonical software-wallet envelope, while operating-
system authentication belongs to clients and only releases a WalletUnlockKey.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass

from .errors import ProtectionError, ProtectionUnavailableError, WalletError
from .secret_store import (
    WalletUnlockKey,
    decrypt_secret,
    decrypt_secret_with_unlock_key,
    derive_unlock_key,
    encrypt_secret,
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
        if envelope.get("cipher") == "aes-256-gcm" and "kdf" in envelope:
            return "password"
        raise WalletError("Unsupported wallet protection format")

    def protect(self, payload: dict, credential: ProtectionCredential) -> dict:
        if credential.kind != "password":
            raise ProtectionError("Only password protection is supported by Core")
        provider = self._provider("password")
        return self.wrap("password", provider.protect(payload, credential))

    def unprotect(self, envelope: dict, credential: ProtectionCredential) -> dict:
        if credential.kind != "password":
            raise ProtectionError("Only password protection is supported by Core")
        provider_envelope = self._password_provider_envelope(envelope)
        return self._provider("password").unprotect(provider_envelope, credential)

    def derive_unlock_key(self, envelope: dict, password: str) -> WalletUnlockKey:
        return derive_unlock_key(self._password_provider_envelope(envelope), password)

    def unprotect_with_unlock_key(
        self, envelope: dict, unlock_key: WalletUnlockKey
    ) -> dict:
        return decrypt_secret_with_unlock_key(
            self._password_provider_envelope(envelope), unlock_key
        )

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

    def _password_provider_envelope(self, envelope: dict) -> dict:
        kind = self.kind_for(envelope)
        if kind != "password":
            raise ProtectionError(f"Unsupported wallet protection kind: {kind}")
        if self.is_legacy_password(envelope):
            return envelope
        payload = envelope.get("payload")
        if not isinstance(payload, dict):
            raise WalletError("Wallet protection payload is corrupted")
        return payload

    def _provider(self, kind: str) -> ProtectionProvider:
        try:
            return self._providers[kind]
        except KeyError as exc:
            raise ProtectionUnavailableError(
                f"Protection provider is not available: {kind}"
            ) from exc

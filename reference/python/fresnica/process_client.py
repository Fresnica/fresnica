"""Process adapter for the production Fresnica process binding.

The adapter is intentionally narrow: Python owns product orchestration, storage,
networking, and UI while the Rust process owns identity parsing, protected
software-signer cryptography, signer identity checks, and signing semantics.
Sensitive inputs are sent over stdin, never command-line args.
"""

from __future__ import annotations

import base64
from dataclasses import dataclass
import json
import os
from pathlib import Path
import shutil
import subprocess

from .errors import (
    InvalidPasswordError,
    InvalidUnlockKeyError,
    SignerError,
    WalletError,
    WalletLockedError,
)
from .secret_store import WalletUnlockKey


PROTOCOL_VERSION = 1


@dataclass(frozen=True)
class AccountIdentityResult:
    address: str
    kind: str
    public_key: str | None


@dataclass(frozen=True)
class ProtectedSoftwareSignerResult:
    signer_public_key: str
    envelope: dict


@dataclass(frozen=True)
class GeneratedMnemonicResult:
    signer_public_key: str
    envelope: dict
    mnemonic: str
    language: str
    index: int


@dataclass(frozen=True)
class Ed25519SigningRequestResult:
    transaction_hash: bytes
    transaction_xdr: str
    network_passphrase: str


@dataclass(frozen=True)
class SorobanAuthorizationSigningRequestResult:
    authorization_hash: bytes
    authorization_entry_xdr: str
    authorization_preimage_xdr: bytes
    network_passphrase: str


class FresnicaProcessUnavailableError(WalletError):
    pass


class _ProcessProtocolError(Exception):
    def __init__(self, code: str, message: str):
        self.code = code
        super().__init__(message)


class FresnicaProcessClient:
    def __init__(self, binary: str | os.PathLike[str]):
        path = Path(binary).expanduser().resolve()
        if not path.is_file() or not os.access(path, os.X_OK):
            raise FresnicaProcessUnavailableError(f"Fresnica process binding binary is not executable: {path}")
        self.binary = path

    @classmethod
    def discover(cls) -> "FresnicaProcessClient | None":
        configured = os.environ.get("FRESNICA_PROCESS_BIN")
        if configured:
            return cls(configured)
        found = shutil.which("fresnica-process")
        return cls(found) if found else None

    def version(self) -> dict:
        return self._call({"operation": "version"})

    def parse_account(self, address: str) -> AccountIdentityResult:
        try:
            result = self._call({"operation": "parse-account", "address": address})
        except _ProcessProtocolError as exc:
            raise WalletError(str(exc)) from exc
        try:
            parsed_address = result["address"]
            kind = result["kind"]
            public_key = result.get("public_key")
        except (KeyError, TypeError) as exc:
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed account identity") from exc
        if not isinstance(parsed_address, str) or kind not in {"classic", "contract"}:
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed account identity")
        if public_key is not None and not isinstance(public_key, str):
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed account identity")
        return AccountIdentityResult(parsed_address, kind, public_key)

    def protect_secret(
        self,
        secret: str,
        passcode: str,
        *,
        expected_signer_public_key: str | None = None,
    ) -> ProtectedSoftwareSignerResult:
        try:
            result = self._call(
                {
                    "operation": "protect-secret",
                    "secret": secret,
                    "passcode": passcode,
                    "expected_signer_public_key": expected_signer_public_key,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise WalletError(str(exc)) from exc
        return _protected_signer_result(result)

    def protect_mnemonic(
        self,
        mnemonic: str,
        passcode: str,
        *,
        mnemonic_passphrase: str = "",
        index: int = 0,
        language: str | None = None,
        expected_signer_public_key: str | None = None,
    ) -> ProtectedSoftwareSignerResult:
        try:
            result = self._call(
                {
                    "operation": "protect-mnemonic",
                    "mnemonic": mnemonic,
                    "mnemonic_passphrase": mnemonic_passphrase,
                    "index": index,
                    "language": language,
                    "passcode": passcode,
                    "expected_signer_public_key": expected_signer_public_key,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise WalletError(str(exc)) from exc
        return _protected_signer_result(result)

    def generate_mnemonic(
        self,
        passcode: str,
        *,
        mnemonic_passphrase: str = "",
        index: int = 0,
        language: str = "english",
        strength: int = 256,
    ) -> GeneratedMnemonicResult:
        try:
            result = self._call(
                {
                    "operation": "generate-mnemonic",
                    "passcode": passcode,
                    "mnemonic_passphrase": mnemonic_passphrase,
                    "index": index,
                    "language": language,
                    "strength": strength,
                }
            )
        except _ProcessProtocolError as exc:
            raise WalletError(str(exc)) from exc
        protected = _protected_signer_result(result)
        try:
            mnemonic = result["mnemonic"]
            language_value = result["language"]
            index_value = int(result["index"])
        except (KeyError, TypeError, ValueError) as exc:
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed mnemonic data") from exc
        if not isinstance(mnemonic, str) or not isinstance(language_value, str):
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed mnemonic data")
        return GeneratedMnemonicResult(
            protected.signer_public_key,
            protected.envelope,
            mnemonic,
            language_value,
            index_value,
        )

    def reprotect(
        self,
        envelope: dict,
        current_passcode: str,
        new_passcode: str,
        expected_signer_public_key: str,
    ) -> ProtectedSoftwareSignerResult:
        try:
            result = self._call(
                {
                    "operation": "reprotect",
                    "envelope": envelope,
                    "current_passcode": current_passcode,
                    "new_passcode": new_passcode,
                    "expected_signer_public_key": expected_signer_public_key,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-passcode":
                raise InvalidPasswordError("Invalid wallet password") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise WalletError(str(exc)) from exc
        return _protected_signer_result(result)

    def derive_verified_unlock_key(
        self,
        envelope: dict,
        passcode: str,
        expected_signer_public_key: str,
    ) -> WalletUnlockKey:
        try:
            result = self._call(
                {
                    "operation": "derive-unlock-key",
                    "envelope": envelope,
                    "passcode": passcode,
                    "expected_signer_public_key": expected_signer_public_key,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-passcode":
                raise InvalidPasswordError("Invalid wallet password") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise WalletError(str(exc)) from exc
        try:
            raw = base64.b64decode(result["unlock_key"], validate=True)
        except (KeyError, TypeError, ValueError) as exc:
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed unlock key data") from exc
        return WalletUnlockKey(raw)

    def validate_unlock_key(
        self,
        envelope: dict,
        unlock_key: WalletUnlockKey,
        expected_signer_public_key: str,
    ) -> None:
        try:
            self._call(
                {
                    "operation": "validate-unlock-key",
                    "envelope": envelope,
                    "unlock_key": base64.b64encode(unlock_key.as_bytes()).decode("ascii"),
                    "expected_signer_public_key": expected_signer_public_key,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-unlock-key":
                raise InvalidUnlockKeyError("Invalid wallet unlock key") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise WalletError(str(exc)) from exc

    def sign_transaction(
        self,
        envelope: dict,
        unlock_key: WalletUnlockKey,
        expected_signer_public_key: str,
        transaction_xdr: str,
        network_passphrase: str,
    ) -> str:
        try:
            result = self._call(
                {
                    "operation": "sign-transaction",
                    "envelope": envelope,
                    "unlock_key": base64.b64encode(unlock_key.as_bytes()).decode("ascii"),
                    "expected_signer_public_key": expected_signer_public_key,
                    "transaction_xdr": transaction_xdr,
                    "network_passphrase": network_passphrase,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-unlock-key":
                raise InvalidUnlockKeyError("Invalid wallet unlock key") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise SignerError(str(exc)) from exc
        return _signed_xdr(result)

    def sign_soroban_authorization(
        self,
        envelope: dict,
        unlock_key: WalletUnlockKey,
        expected_signer_public_key: str,
        authorization_entry_xdr: str,
        network_passphrase: str,
    ) -> str:
        try:
            result = self._call(
                {
                    "operation": "sign-soroban-authorization",
                    "envelope": envelope,
                    "unlock_key": base64.b64encode(unlock_key.as_bytes()).decode("ascii"),
                    "expected_signer_public_key": expected_signer_public_key,
                    "authorization_entry_xdr": authorization_entry_xdr,
                    "network_passphrase": network_passphrase,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-unlock-key":
                raise InvalidUnlockKeyError("Invalid wallet unlock key") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise SignerError(str(exc)) from exc
        return _signed_authorization_xdr(result)

    def sign_soroban_authorization_with_passcode(
        self,
        envelope: dict,
        passcode: str,
        expected_signer_public_key: str,
        authorization_entry_xdr: str,
        network_passphrase: str,
    ) -> str:
        try:
            result = self._call(
                {
                    "operation": "sign-soroban-authorization-with-passcode",
                    "envelope": envelope,
                    "passcode": passcode,
                    "expected_signer_public_key": expected_signer_public_key,
                    "authorization_entry_xdr": authorization_entry_xdr,
                    "network_passphrase": network_passphrase,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-passcode":
                raise InvalidPasswordError("Invalid wallet password") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise SignerError(str(exc)) from exc
        return _signed_authorization_xdr(result)

    def reveal(self, envelope: dict, passcode: str, expected_signer_public_key: str) -> dict:
        try:
            result = self._call(
                {
                    "operation": "reveal",
                    "envelope": envelope,
                    "passcode": passcode,
                    "expected_signer_public_key": expected_signer_public_key,
                }
            )
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-passcode":
                raise InvalidPasswordError("Invalid wallet password") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise WalletError(str(exc)) from exc
        if not isinstance(result, dict) or result.get("kind") not in {"secret", "mnemonic"}:
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed signing material")
        return result

    def prepare_ed25519_signing(
        self,
        transaction_xdr: str,
        network_passphrase: str,
    ) -> Ed25519SigningRequestResult:
        try:
            result = self._call(
                {
                    "operation": "prepare-ed25519-signing",
                    "transaction_xdr": transaction_xdr,
                    "network_passphrase": network_passphrase,
                }
            )
        except _ProcessProtocolError as exc:
            raise SignerError(str(exc)) from exc
        try:
            transaction_hash = base64.b64decode(result["transaction_hash"], validate=True)
            normalized_xdr = result["transaction_xdr"]
            returned_network = result["network_passphrase"]
        except (KeyError, TypeError, ValueError) as exc:
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed signing request") from exc
        if len(transaction_hash) != 32 or not isinstance(normalized_xdr, str) or not isinstance(returned_network, str):
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed signing request")
        return Ed25519SigningRequestResult(transaction_hash, normalized_xdr, returned_network)

    def apply_ed25519_signature(
        self,
        transaction_xdr: str,
        network_passphrase: str,
        signer_public_key: str,
        signature: bytes,
    ) -> str:
        if not isinstance(signature, bytes) or len(signature) != 64:
            raise SignerError("External signer must return a 64-byte Ed25519 signature")
        try:
            result = self._call(
                {
                    "operation": "apply-ed25519-signature",
                    "transaction_xdr": transaction_xdr,
                    "network_passphrase": network_passphrase,
                    "signer_public_key": signer_public_key,
                    "signature": base64.b64encode(signature).decode("ascii"),
                }
            )
        except _ProcessProtocolError as exc:
            raise SignerError(str(exc)) from exc
        return _signed_xdr(result)


    def prepare_soroban_authorization_signing(
        self,
        authorization_entry_xdr: str,
        network_passphrase: str,
    ) -> SorobanAuthorizationSigningRequestResult:
        try:
            result = self._call(
                {
                    "operation": "prepare-soroban-authorization-signing",
                    "authorization_entry_xdr": authorization_entry_xdr,
                    "network_passphrase": network_passphrase,
                }
            )
        except _ProcessProtocolError as exc:
            raise SignerError(str(exc)) from exc
        try:
            authorization_hash = base64.b64decode(
                result["authorization_hash"], validate=True
            )
            normalized_xdr = result["authorization_entry_xdr"]
            authorization_preimage_xdr = base64.b64decode(
                result["authorization_preimage_xdr"], validate=True
            )
            returned_network = result["network_passphrase"]
        except (KeyError, TypeError, ValueError) as exc:
            raise FresnicaProcessUnavailableError(
                "Fresnica process binding returned malformed Soroban signing request"
            ) from exc
        if (
            len(authorization_hash) != 32
            or not isinstance(normalized_xdr, str)
            or not isinstance(returned_network, str)
        ):
            raise FresnicaProcessUnavailableError(
                "Fresnica process binding returned malformed Soroban signing request"
            )
        return SorobanAuthorizationSigningRequestResult(
            authorization_hash,
            normalized_xdr,
            authorization_preimage_xdr,
            returned_network,
        )

    def apply_soroban_ed25519_signature(
        self,
        authorization_entry_xdr: str,
        network_passphrase: str,
        signer_public_key: str,
        signature: bytes,
    ) -> str:
        if not isinstance(signature, bytes) or len(signature) != 64:
            raise SignerError("External signer must return a 64-byte Ed25519 signature")
        try:
            result = self._call(
                {
                    "operation": "apply-soroban-ed25519-signature",
                    "authorization_entry_xdr": authorization_entry_xdr,
                    "network_passphrase": network_passphrase,
                    "signer_public_key": signer_public_key,
                    "signature": base64.b64encode(signature).decode("ascii"),
                }
            )
        except _ProcessProtocolError as exc:
            raise SignerError(str(exc)) from exc
        return _signed_authorization_xdr(result)

    def derive_mnemonic_signer(self, envelope: dict, passcode: str, expected_signer_public_key: str, index: int) -> ProtectedSoftwareSignerResult:
        try:
            result = self._call({"operation":"derive-mnemonic-signer","envelope":envelope,"passcode":passcode,"expected_signer_public_key":expected_signer_public_key,"index":index})
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-passcode": raise InvalidPasswordError("Invalid wallet password") from exc
            if exc.code == "identity-mismatch": raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise WalletError(str(exc)) from exc
        return _protected_signer_result(result)

    def sign_transaction_with_passcode(self, envelope: dict, passcode: str, expected_signer_public_key: str, transaction_xdr: str, network_passphrase: str) -> str:
        try:
            result = self._call({"operation":"sign-transaction-with-passcode","envelope":envelope,"passcode":passcode,"expected_signer_public_key":expected_signer_public_key,"transaction_xdr":transaction_xdr,"network_passphrase":network_passphrase})
        except _ProcessProtocolError as exc:
            if exc.code == "invalid-passcode": raise InvalidPasswordError("Invalid wallet password") from exc
            if exc.code == "identity-mismatch": raise WalletLockedError("Signer identity does not match expected signer") from exc
            raise SignerError(str(exc)) from exc
        return _signed_xdr(result)

    def _call(self, request: dict) -> dict:
        encoded = json.dumps(
            request,
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode("utf-8")
        try:
            completed = subprocess.run(
                [str(self.binary)],
                input=encoded,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                check=False,
            )
        except OSError as exc:
            raise FresnicaProcessUnavailableError(
                f"Unable to execute Fresnica process binding at {self.binary}: {exc}"
            ) from exc
        try:
            response = json.loads(completed.stdout.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise FresnicaProcessUnavailableError("Fresnica process binding returned invalid protocol data") from exc
        if not isinstance(response, dict) or response.get("protocol_version") != PROTOCOL_VERSION:
            raise FresnicaProcessUnavailableError("Fresnica process binding protocol version mismatch")
        if response.get("ok") is True:
            result = response.get("result")
            if isinstance(result, dict):
                return result
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed protocol data")
        error = response.get("error")
        if not isinstance(error, dict):
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed error data")
        code = error.get("code")
        message = error.get("message")
        if not isinstance(code, str) or not isinstance(message, str):
            raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed error data")
        raise _ProcessProtocolError(code, message)


def _protected_signer_result(result: dict) -> ProtectedSoftwareSignerResult:
    try:
        signer_public_key = result["signer_public_key"]
        envelope = result["envelope"]
    except (KeyError, TypeError) as exc:
        raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed protected signer data") from exc
    if not isinstance(signer_public_key, str) or not isinstance(envelope, dict):
        raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed protected signer data")
    return ProtectedSoftwareSignerResult(signer_public_key, envelope)


def _signed_xdr(result: dict) -> str:
    try:
        signed_xdr = result["transaction_xdr"]
    except (KeyError, TypeError) as exc:
        raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed signed transaction") from exc
    if not isinstance(signed_xdr, str):
        raise FresnicaProcessUnavailableError("Fresnica process binding returned malformed signed transaction")
    return signed_xdr


def _signed_authorization_xdr(result: dict) -> str:
    try:
        signed_xdr = result["authorization_entry_xdr"]
    except (KeyError, TypeError) as exc:
        raise FresnicaProcessUnavailableError(
            "Fresnica process binding returned malformed signed Soroban authorization"
        ) from exc
    if not isinstance(signed_xdr, str):
        raise FresnicaProcessUnavailableError(
            "Fresnica process binding returned malformed signed Soroban authorization"
        )
    return signed_xdr

"""Process adapter for the production Rust Core.

The adapter is intentionally narrow: Python owns product orchestration, storage,
networking, and UI while the Rust process owns protected wallet material and
software signing. Sensitive inputs are sent over stdin, never command-line args.
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
class ProtectedWalletResult:
    public_key: str
    envelope: dict


@dataclass(frozen=True)
class GeneratedMnemonicResult:
    public_key: str
    envelope: dict
    mnemonic: str
    language: str
    index: int


class RustCoreUnavailableError(WalletError):
    pass


class _RustCoreProtocolError(Exception):
    def __init__(self, code: str, message: str):
        self.code = code
        super().__init__(message)


class RustCoreClient:
    def __init__(self, binary: str | os.PathLike[str]):
        path = Path(binary).expanduser().resolve()
        if not path.is_file() or not os.access(path, os.X_OK):
            raise RustCoreUnavailableError(f"Rust Core binary is not executable: {path}")
        self.binary = path

    @classmethod
    def discover(cls) -> "RustCoreClient | None":
        configured = os.environ.get("FRESNICA_CORE_BIN")
        if configured:
            return cls(configured)
        found = shutil.which("fresnica-core")
        return cls(found) if found else None

    def version(self) -> dict:
        return self._call({"operation": "version"})

    def protect_secret(self, secret: str, passcode: str) -> ProtectedWalletResult:
        try:
            result = self._call(
                {
                    "operation": "protect-secret",
                    "secret": secret,
                    "passcode": passcode,
                }
            )
        except _RustCoreProtocolError as exc:
            raise WalletError(str(exc)) from exc
        return _protected_wallet_result(result)

    def protect_mnemonic(
        self,
        mnemonic: str,
        passcode: str,
        *,
        mnemonic_passphrase: str = "",
        index: int = 0,
        language: str | None = None,
    ) -> ProtectedWalletResult:
        try:
            result = self._call(
                {
                    "operation": "protect-mnemonic",
                    "mnemonic": mnemonic,
                    "mnemonic_passphrase": mnemonic_passphrase,
                    "index": index,
                    "language": language,
                    "passcode": passcode,
                }
            )
        except _RustCoreProtocolError as exc:
            raise WalletError(str(exc)) from exc
        return _protected_wallet_result(result)

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
        except _RustCoreProtocolError as exc:
            raise WalletError(str(exc)) from exc
        protected = _protected_wallet_result(result)
        try:
            mnemonic = result["mnemonic"]
            language_value = result["language"]
            index_value = int(result["index"])
        except (KeyError, TypeError, ValueError) as exc:
            raise RustCoreUnavailableError("Rust Core returned malformed mnemonic data") from exc
        if not isinstance(mnemonic, str) or not isinstance(language_value, str):
            raise RustCoreUnavailableError("Rust Core returned malformed mnemonic data")
        return GeneratedMnemonicResult(
            protected.public_key,
            protected.envelope,
            mnemonic,
            language_value,
            index_value,
        )

    def derive_verified_unlock_key(
        self,
        envelope: dict,
        passcode: str,
        expected_public_key: str,
    ) -> WalletUnlockKey:
        try:
            result = self._call(
                {
                    "operation": "derive-unlock-key",
                    "envelope": envelope,
                    "passcode": passcode,
                    "expected_public_key": expected_public_key,
                }
            )
        except _RustCoreProtocolError as exc:
            if exc.code == "invalid-passcode":
                raise InvalidPasswordError("Invalid wallet password") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Decrypted wallet identity does not match metadata") from exc
            raise WalletError(str(exc)) from exc
        try:
            raw = base64.b64decode(result["unlock_key"], validate=True)
        except (KeyError, TypeError, ValueError) as exc:
            raise RustCoreUnavailableError("Rust Core returned malformed unlock key data") from exc
        return WalletUnlockKey(raw)

    def validate_unlock_key(
        self,
        envelope: dict,
        unlock_key: WalletUnlockKey,
        expected_public_key: str,
    ) -> None:
        try:
            self._call(
                {
                    "operation": "validate-unlock-key",
                    "envelope": envelope,
                    "unlock_key": base64.b64encode(unlock_key.as_bytes()).decode("ascii"),
                    "expected_public_key": expected_public_key,
                }
            )
        except _RustCoreProtocolError as exc:
            if exc.code == "invalid-unlock-key":
                raise InvalidUnlockKeyError("Invalid wallet unlock key") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Decrypted wallet identity does not match metadata") from exc
            raise WalletError(str(exc)) from exc

    def sign_transaction(
        self,
        envelope: dict,
        unlock_key: WalletUnlockKey,
        expected_public_key: str,
        transaction_xdr: str,
        network_passphrase: str,
    ) -> str:
        try:
            result = self._call(
                {
                    "operation": "sign-transaction",
                    "envelope": envelope,
                    "unlock_key": base64.b64encode(unlock_key.as_bytes()).decode("ascii"),
                    "expected_public_key": expected_public_key,
                    "transaction_xdr": transaction_xdr,
                    "network_passphrase": network_passphrase,
                }
            )
        except _RustCoreProtocolError as exc:
            if exc.code == "invalid-unlock-key":
                raise InvalidUnlockKeyError("Invalid wallet unlock key") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Decrypted wallet identity does not match metadata") from exc
            raise SignerError(str(exc)) from exc
        try:
            signed_xdr = result["transaction_xdr"]
        except (KeyError, TypeError) as exc:
            raise RustCoreUnavailableError("Rust Core returned malformed signed transaction") from exc
        if not isinstance(signed_xdr, str):
            raise RustCoreUnavailableError("Rust Core returned malformed signed transaction")
        return signed_xdr

    def reveal(self, envelope: dict, passcode: str, expected_public_key: str) -> dict:
        try:
            result = self._call(
                {
                    "operation": "reveal",
                    "envelope": envelope,
                    "passcode": passcode,
                    "expected_public_key": expected_public_key,
                }
            )
        except _RustCoreProtocolError as exc:
            if exc.code == "invalid-passcode":
                raise InvalidPasswordError("Invalid wallet password") from exc
            if exc.code == "identity-mismatch":
                raise WalletLockedError("Decrypted wallet identity does not match metadata") from exc
            raise WalletError(str(exc)) from exc
        if not isinstance(result, dict) or result.get("kind") not in {"secret", "mnemonic"}:
            raise RustCoreUnavailableError("Rust Core returned malformed signing material")
        return result

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
            raise RustCoreUnavailableError(
                f"Unable to execute Rust Core at {self.binary}: {exc}"
            ) from exc
        try:
            response = json.loads(completed.stdout.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise RustCoreUnavailableError("Rust Core returned invalid protocol data") from exc
        if not isinstance(response, dict) or response.get("protocol_version") != PROTOCOL_VERSION:
            raise RustCoreUnavailableError("Rust Core protocol version mismatch")
        if response.get("ok") is True:
            result = response.get("result")
            if isinstance(result, dict):
                return result
            raise RustCoreUnavailableError("Rust Core returned malformed protocol data")
        error = response.get("error")
        if not isinstance(error, dict):
            raise RustCoreUnavailableError("Rust Core returned malformed error data")
        code = error.get("code")
        message = error.get("message")
        if not isinstance(code, str) or not isinstance(message, str):
            raise RustCoreUnavailableError("Rust Core returned malformed error data")
        raise _RustCoreProtocolError(code, message)


def _protected_wallet_result(result: dict) -> ProtectedWalletResult:
    try:
        public_key = result["public_key"]
        envelope = result["envelope"]
    except (KeyError, TypeError) as exc:
        raise RustCoreUnavailableError("Rust Core returned malformed protected wallet data") from exc
    if not isinstance(public_key, str) or not isinstance(envelope, dict):
        raise RustCoreUnavailableError("Rust Core returned malformed protected wallet data")
    return ProtectedWalletResult(public_key, envelope)

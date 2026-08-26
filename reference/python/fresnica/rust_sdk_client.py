"""Process adapter for validating the platform-neutral Fresnica Rust SDK from Python."""

from __future__ import annotations

import os
import shutil

from .rust_core_client import (
    ProtectedSoftwareSignerResult,
    RustCoreClient,
    RustCoreUnavailableError,
    _protected_signer_result,
    _signed_xdr,
)
from .errors import InvalidPasswordError, SignerError, WalletError, WalletLockedError


class RustSdkClient(RustCoreClient):
    """Same narrow process transport as ``RustCoreClient``, but backed by FresnicaSdk."""

    @classmethod
    def discover(cls) -> "RustSdkClient | None":
        configured = os.environ.get("FRESNICA_SDK_BIN")
        if configured:
            return cls(configured)
        found = shutil.which("fresnica-sdk-bridge")
        return cls(found) if found else None

    def derive_mnemonic_signer(
        self,
        envelope: dict,
        passcode: str,
        expected_signer_public_key: str,
        index: int,
    ) -> ProtectedSoftwareSignerResult:
        try:
            result = self._call(
                {
                    "operation": "derive-mnemonic-signer",
                    "envelope": envelope,
                    "passcode": passcode,
                    "expected_signer_public_key": expected_signer_public_key,
                    "index": index,
                }
            )
        except Exception as exc:
            code = getattr(exc, "code", None)
            if code == "invalid-passcode":
                raise InvalidPasswordError("Invalid wallet password") from exc
            if code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            if isinstance(exc, RustCoreUnavailableError):
                raise
            raise WalletError(str(exc)) from exc
        return _protected_signer_result(result)

    def sign_transaction_with_passcode(
        self,
        envelope: dict,
        passcode: str,
        expected_signer_public_key: str,
        transaction_xdr: str,
        network_passphrase: str,
    ) -> str:
        try:
            result = self._call(
                {
                    "operation": "sign-transaction-with-passcode",
                    "envelope": envelope,
                    "passcode": passcode,
                    "expected_signer_public_key": expected_signer_public_key,
                    "transaction_xdr": transaction_xdr,
                    "network_passphrase": network_passphrase,
                }
            )
        except Exception as exc:
            code = getattr(exc, "code", None)
            if code == "invalid-passcode":
                raise InvalidPasswordError("Invalid wallet password") from exc
            if code == "identity-mismatch":
                raise WalletLockedError("Signer identity does not match expected signer") from exc
            if isinstance(exc, RustCoreUnavailableError):
                raise
            raise SignerError(str(exc)) from exc
        return _signed_xdr(result)

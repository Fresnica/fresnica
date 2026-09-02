"""Ledger Stellar application transport for RefPython hardware-signer proofs.

This module deliberately keeps Ledger HID/APDU behavior above Fresnica Core.
Core remains authoritative for the exact transaction hash, signer identity, and
signature verification through the external Ed25519 prepare/apply boundary.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from stellar_sdk import StrKey, TransactionEnvelope

from .errors import SignerError
from .signer import ExternalSigningRequest


DEFAULT_LEDGER_STELLAR_PATH = "m/44'/148'/0'"

_CLA = 0xE0
_INS_GET_PUBLIC_KEY = 0x02
_INS_SIGN_TX = 0x04
_INS_GET_CONFIGURATION = 0x06
_P1_FIRST = 0x00
_P1_MORE = 0x80
_P2_LAST = 0x00
_P2_MORE = 0x80
_P2_NO_CONFIRM = 0x00
_P2_CONFIRM = 0x01
_SW_OK = 0x9000
_SW_DENY = 0x6985
_SW_APP_NOT_OPEN = 0x6D00
_SW_BLIND_SIGNING_REQUIRED = 0x6C66
_MAX_APDU_DATA = 255


class _LedgerTransport(Protocol):
    def exchange(
        self,
        cla: int,
        ins: int,
        p1: int = 0,
        p2: int = 0,
        option: int | None = None,
        cdata: bytes = b"",
    ) -> tuple[int, bytes]: ...

    def close(self) -> None: ...


@dataclass(frozen=True)
class LedgerStellarConfiguration:
    version: str
    blind_signing_enabled: bool
    max_data_size: int


class LedgerStellarError(SignerError):
    pass


class LedgerStellarUserRejected(LedgerStellarError):
    pass


class LedgerStellarProvider:
    """Small RefPython provider for the official Ledger Stellar application.

    The APDU shape follows LedgerHQ/app-stellar. The transport itself is the
    maintained ``ledgercomm`` HID implementation; importing this module does
    not require Ledger dependencies until a real HID provider is opened.
    """

    def __init__(
        self,
        *,
        path: str = DEFAULT_LEDGER_STELLAR_PATH,
        transport: _LedgerTransport | None = None,
        debug: bool = False,
    ):
        self.path = path
        self._transport = transport or _open_hid_transport(debug=debug)
        self._owns_transport = transport is None

    def close(self) -> None:
        if self._owns_transport:
            self._transport.close()

    def __enter__(self) -> "LedgerStellarProvider":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def get_configuration(self) -> LedgerStellarConfiguration:
        sw, response = self._transport.exchange(
            cla=_CLA,
            ins=_INS_GET_CONFIGURATION,
            p1=_P1_FIRST,
            p2=_P2_LAST,
        )
        _require_ok(sw, "read Stellar app configuration")
        if len(response) != 6:
            raise LedgerStellarError(
                f"Ledger Stellar app returned malformed configuration ({len(response)} bytes)"
            )
        return LedgerStellarConfiguration(
            version=f"{response[1]}.{response[2]}.{response[3]}",
            blind_signing_enabled=response[0] == 0x01,
            max_data_size=(response[4] << 8) | response[5],
        )

    def get_public_key(self, *, confirm_on_device: bool = False) -> str:
        sw, response = self._transport.exchange(
            cla=_CLA,
            ins=_INS_GET_PUBLIC_KEY,
            p1=_P1_FIRST,
            p2=_P2_CONFIRM if confirm_on_device else _P2_NO_CONFIRM,
            cdata=_pack_derivation_path(self.path),
        )
        _require_ok(sw, "read Stellar public key")
        if len(response) != 32:
            raise LedgerStellarError(
                f"Ledger Stellar app returned malformed public key ({len(response)} bytes)"
            )
        return StrKey.encode_ed25519_public_key(response)

    def sign_request(self, request: ExternalSigningRequest) -> bytes:
        """Sign one Core-prepared transaction through Ledger clear signing.

        Fresnica Core supplies normalized XDR + network context. The provider
        converts that public representation to the Stellar signature-base bytes
        expected by Ledger's ``SIGN_TX`` command. Core verifies the returned
        signature against the original normalized transaction before accepting
        it into signed state.
        """
        try:
            envelope = TransactionEnvelope.from_xdr(
                request.transaction_xdr,
                request.network_passphrase,
            )
        except Exception as exc:
            raise LedgerStellarError(
                "Unable to decode Core-prepared transaction for Ledger signing"
            ) from exc

        if envelope.hash() != request.transaction_hash:
            raise LedgerStellarError(
                "Core-prepared Ledger transaction hash does not match transaction XDR"
            )

        signature_base = envelope.signature_base()
        return self.sign_transaction_signature_base(signature_base)

    def sign_transaction_signature_base(self, signature_base: bytes) -> bytes:
        if not signature_base:
            raise LedgerStellarError("Ledger transaction signature base must not be empty")

        packed_path = _pack_derivation_path(self.path)
        sw, response = self._transport.exchange(
            cla=_CLA,
            ins=_INS_SIGN_TX,
            p1=_P1_FIRST,
            p2=_P2_MORE,
            cdata=packed_path,
        )
        _require_ok(sw, "start Stellar transaction signing")
        if response:
            raise LedgerStellarError("Ledger returned unexpected data before transaction review")

        chunks = _split_chunks(signature_base, _MAX_APDU_DATA)
        for chunk in chunks[:-1]:
            sw, response = self._transport.exchange(
                cla=_CLA,
                ins=_INS_SIGN_TX,
                p1=_P1_MORE,
                p2=_P2_MORE,
                cdata=chunk,
            )
            _require_ok(sw, "stream Stellar transaction to device")
            if response:
                raise LedgerStellarError("Ledger returned unexpected data before final transaction chunk")

        sw, signature = self._transport.exchange(
            cla=_CLA,
            ins=_INS_SIGN_TX,
            p1=_P1_MORE,
            p2=_P2_LAST,
            cdata=chunks[-1],
        )
        _require_ok(sw, "approve Stellar transaction on device")
        if len(signature) != 64:
            raise LedgerStellarError(
                f"Ledger Stellar app returned malformed signature ({len(signature)} bytes)"
            )
        return signature


def _open_hid_transport(*, debug: bool) -> _LedgerTransport:
    try:
        from ledgercomm import Transport
    except ImportError as exc:
        raise LedgerStellarError(
            'Ledger HID support is not installed; run `uv pip install -r requirements-ledger.txt` in reference/python'
        ) from exc

    try:
        return Transport(interface="hid", debug=debug)
    except Exception as exc:
        raise LedgerStellarError(
            "Unable to open Ledger over HID; unlock the device, open the Stellar app, and close Ledger Live"
        ) from exc


def _split_chunks(payload: bytes, size: int) -> list[bytes]:
    if size <= 0:
        raise ValueError("chunk size must be positive")
    return [payload[offset : offset + size] for offset in range(0, len(payload), size)]


def _pack_derivation_path(path: str) -> bytes:
    text = path.strip()
    if text == "m":
        return b"\x00"
    if not text.startswith("m/"):
        raise LedgerStellarError("Ledger derivation path must start with m/")

    components = text[2:].split("/")
    if not components or any(not component for component in components):
        raise LedgerStellarError("Ledger derivation path is malformed")
    if len(components) > 10:
        raise LedgerStellarError("Ledger derivation path is too deep")

    encoded = bytearray([len(components)])
    for component in components:
        hardened = component.endswith("'")
        number_text = component[:-1] if hardened else component
        if not number_text.isdigit():
            raise LedgerStellarError("Ledger derivation path contains a non-numeric component")
        number = int(number_text)
        if number >= 0x80000000:
            raise LedgerStellarError("Ledger derivation path component is out of range")
        if hardened:
            number |= 0x80000000
        encoded.extend(number.to_bytes(4, "big"))
    return bytes(encoded)


def _require_ok(status_word: int, action: str) -> None:
    if status_word == _SW_OK:
        return
    if status_word == _SW_DENY:
        raise LedgerStellarUserRejected(f"Ledger user rejected request while trying to {action}")
    if status_word == _SW_APP_NOT_OPEN:
        raise LedgerStellarError(
            f"Ledger Stellar app did not accept the command while trying to {action}; make sure the Stellar app is open"
        )
    if status_word == _SW_BLIND_SIGNING_REQUIRED:
        raise LedgerStellarError(
            f"Ledger requires blind signing for this request while trying to {action}"
        )
    raise LedgerStellarError(
        f"Ledger Stellar app failed to {action} with status 0x{status_word:04X}"
    )

"""Encryption helpers for wallet secret material.

Fresnica uses established cryptographic primitives from ``cryptography``:
Scrypt for password-based key derivation and AES-256-GCM for authenticated
encryption. Public wallet metadata is not encrypted.
"""

import base64
import binascii
import json
import os

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.primitives.kdf.scrypt import Scrypt

from .errors import InvalidPasswordError, InvalidUnlockKeyError, WalletError


AAD = b"fresnica-wallet-secret-v1"
KEY_AAD = b"fresnica-wallet-secret-key-v1"
SCRYPT_N = 2**15
SCRYPT_R = 8
SCRYPT_P = 1


class WalletUnlockKey:
    """Reference representation of the canonical 32-byte wallet unlock key."""

    __slots__ = ("_value",)

    def __init__(self, value: bytes):
        if not isinstance(value, bytes) or len(value) != 32:
            raise WalletError("Wallet unlock key must be 32 bytes")
        self._value = value

    def as_bytes(self) -> bytes:
        return self._value

    def __repr__(self) -> str:
        return "WalletUnlockKey(<redacted>)"


def _b64(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


def _unb64(data: str) -> bytes:
    return base64.b64decode(data.encode("ascii"), validate=True)


def _derive_key(password: str, salt: bytes, n: int, r: int, p: int) -> bytes:
    if not password:
        raise WalletError("Wallet password cannot be empty")
    return Scrypt(salt=salt, length=32, n=n, r=r, p=p).derive(
        password.encode("utf-8")
    )


def _encode_payload(payload: dict) -> bytes:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def _decode_payload(plaintext: bytes) -> dict:
    return json.loads(plaintext.decode("utf-8"))


def _password_envelope_material(envelope: dict) -> tuple[bytes, bytes, bytes, int, int, int]:
    try:
        if envelope.get("version") != 1 or envelope.get("cipher") != "aes-256-gcm":
            raise WalletError("Unsupported wallet encryption format")
        kdf = envelope["kdf"]
        if kdf.get("name") != "scrypt":
            raise WalletError("Unsupported wallet key derivation format")
        n = int(kdf["n"])
        r = int(kdf["r"])
        p = int(kdf["p"])
        if (n, r, p) != (SCRYPT_N, SCRYPT_R, SCRYPT_P):
            raise WalletError("Unsupported wallet KDF parameters")
        salt = _unb64(kdf["salt"])
        nonce = _unb64(envelope["nonce"])
        ciphertext = _unb64(envelope["ciphertext"])
        if len(salt) != 16 or len(nonce) != 12:
            raise ValueError
        return salt, nonce, ciphertext, n, r, p
    except (KeyError, ValueError, TypeError, binascii.Error) as exc:
        raise WalletError("Wallet secret data is corrupted") from exc


def encrypt_secret(payload: dict, password: str) -> dict:
    salt = os.urandom(16)
    nonce = os.urandom(12)
    key = _derive_key(password, salt, SCRYPT_N, SCRYPT_R, SCRYPT_P)
    ciphertext = AESGCM(key).encrypt(nonce, _encode_payload(payload), AAD)
    return {
        "version": 1,
        "cipher": "aes-256-gcm",
        "kdf": {
            "name": "scrypt",
            "n": SCRYPT_N,
            "r": SCRYPT_R,
            "p": SCRYPT_P,
            "salt": _b64(salt),
        },
        "nonce": _b64(nonce),
        "ciphertext": _b64(ciphertext),
    }


def derive_unlock_key(envelope: dict, password: str) -> WalletUnlockKey:
    salt, _nonce, _ciphertext, n, r, p = _password_envelope_material(envelope)
    return WalletUnlockKey(_derive_key(password, salt, n, r, p))


def decrypt_secret_with_unlock_key(
    envelope: dict, unlock_key: WalletUnlockKey
) -> dict:
    if not isinstance(unlock_key, WalletUnlockKey):
        raise WalletError("Wallet unlock key is invalid")
    _salt, nonce, ciphertext, _n, _r, _p = _password_envelope_material(envelope)
    try:
        plaintext = AESGCM(unlock_key.as_bytes()).decrypt(nonce, ciphertext, AAD)
        return _decode_payload(plaintext)
    except InvalidTag as exc:
        raise InvalidUnlockKeyError("Invalid wallet unlock key") from exc
    except (ValueError, TypeError, json.JSONDecodeError) as exc:
        raise WalletError("Wallet secret data is corrupted") from exc


def decrypt_secret(envelope: dict, password: str) -> dict:
    unlock_key = derive_unlock_key(envelope, password)
    try:
        return decrypt_secret_with_unlock_key(envelope, unlock_key)
    except InvalidUnlockKeyError as exc:
        raise InvalidPasswordError("Invalid wallet password") from exc


# Low-level key-based AEAD helpers remain only for historical test-vector
# compatibility. They are not a system-authentication or wallet-protection API.
def encrypt_secret_with_key(payload: dict, key: bytes) -> dict:
    if not isinstance(key, bytes) or len(key) != 32:
        raise WalletError("Wallet protection key must be 32 bytes")
    nonce = os.urandom(12)
    ciphertext = AESGCM(key).encrypt(nonce, _encode_payload(payload), KEY_AAD)
    return {
        "version": 1,
        "cipher": "aes-256-gcm",
        "nonce": _b64(nonce),
        "ciphertext": _b64(ciphertext),
    }


def decrypt_secret_with_key(envelope: dict, key: bytes) -> dict:
    try:
        if not isinstance(key, bytes) or len(key) != 32:
            raise WalletError("Wallet protection key must be 32 bytes")
        if envelope.get("version") != 1 or envelope.get("cipher") != "aes-256-gcm":
            raise WalletError("Unsupported wallet encryption format")
        nonce = _unb64(envelope["nonce"])
        ciphertext = _unb64(envelope["ciphertext"])
        plaintext = AESGCM(key).decrypt(nonce, ciphertext, KEY_AAD)
        return _decode_payload(plaintext)
    except InvalidTag as exc:
        raise WalletError("Protected wallet secret failed authentication") from exc
    except (KeyError, ValueError, TypeError, json.JSONDecodeError, binascii.Error) as exc:
        raise WalletError("Wallet secret data is corrupted") from exc

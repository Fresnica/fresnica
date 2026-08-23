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

from .errors import InvalidPasswordError, WalletError


AAD = b"fresnica-wallet-secret-v1"
KEY_AAD = b"fresnica-wallet-secret-key-v1"
SCRYPT_N = 2**15
SCRYPT_R = 8
SCRYPT_P = 1


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


def decrypt_secret(envelope: dict, password: str) -> dict:
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
        key = _derive_key(password, salt, n, r, p)
        plaintext = AESGCM(key).decrypt(nonce, ciphertext, AAD)
        return _decode_payload(plaintext)
    except InvalidTag as exc:
        raise InvalidPasswordError("Invalid wallet password") from exc
    except (KeyError, ValueError, TypeError, json.JSONDecodeError, binascii.Error) as exc:
        raise WalletError("Wallet secret data is corrupted") from exc


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

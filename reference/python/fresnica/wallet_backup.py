"""Versioned export/import for encrypted Fresnica wallet records."""

import json
import os
from pathlib import Path

from .errors import NetworkError, WalletError
from .network import get_network
from .storage import WalletRecord
from .wallet import Wallet


BACKUP_FORMAT = "fresnica-wallet-backup"
BACKUP_VERSION = 1


def write_wallet_backup(
    record: WalletRecord,
    path: str | Path,
    *,
    overwrite: bool = False,
) -> Path:
    """Write one wallet record without decrypting its signing material."""
    destination = Path(path).expanduser()
    if destination.exists() and not overwrite:
        raise WalletError(f"Backup file already exists: {destination}")

    _validate_record(record)
    payload = {
        "format": BACKUP_FORMAT,
        "version": BACKUP_VERSION,
        "wallet": record.to_dict(),
    }
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(destination.suffix + ".tmp")
    try:
        temporary.write_text(
            json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        try:
            os.chmod(temporary, 0o600)
        except OSError:
            pass
        os.replace(temporary, destination)
    except OSError as exc:
        temporary.unlink(missing_ok=True)
        raise WalletError(f"Unable to write wallet backup: {destination}") from exc
    return destination


def read_wallet_backup(path: str | Path) -> WalletRecord:
    source = Path(path).expanduser()
    try:
        raw = json.loads(source.read_text(encoding="utf-8"))
    except (OSError, ValueError, TypeError) as exc:
        raise WalletError(f"Unable to read wallet backup: {source}") from exc

    if not isinstance(raw, dict):
        raise WalletError("Invalid wallet backup: expected a JSON object")
    if raw.get("format") != BACKUP_FORMAT or raw.get("version") != BACKUP_VERSION:
        raise WalletError("Unsupported wallet backup format")
    wallet = raw.get("wallet")
    if not isinstance(wallet, dict):
        raise WalletError("Invalid wallet backup: missing wallet record")
    try:
        record = WalletRecord.from_dict(wallet)
    except (KeyError, TypeError, ValueError) as exc:
        raise WalletError("Invalid wallet backup: malformed wallet record") from exc
    _validate_record(record)
    return record


def _validate_record(record: WalletRecord) -> None:
    if not isinstance(record.name, str) or not record.name.strip():
        raise WalletError("Invalid wallet backup: wallet name is missing")
    if record.wallet_type not in {"watch-only", "secret", "mnemonic"}:
        raise WalletError(f"Invalid wallet backup: unsupported wallet type {record.wallet_type}")
    try:
        get_network(record.network)
    except NetworkError as exc:
        raise WalletError(f"Invalid wallet backup: unknown network {record.network}") from exc
    try:
        wallet = Wallet.from_address(record.address)
    except (TypeError, ValueError) as exc:
        raise WalletError("Invalid wallet backup: invalid Stellar address") from exc
    if wallet.address() != record.address:
        raise WalletError("Invalid wallet backup: non-canonical Stellar address")
    if record.watch_only:
        if record.secret is not None:
            raise WalletError("Invalid wallet backup: watch-only wallet contains signing material")
    elif not isinstance(record.secret, dict):
        raise WalletError("Invalid wallet backup: encrypted signing material is missing")
    if not isinstance(record.metadata, dict):
        raise WalletError("Invalid wallet backup: metadata must be an object")

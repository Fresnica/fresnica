"""Persistent recovery for transaction submissions with an uncertain HTTP result."""

from dataclasses import asdict, dataclass
from datetime import datetime, timezone
import json
from pathlib import Path
from typing import Literal

from .errors import TransactionError, TransactionPendingError


PendingStatus = Literal["pending", "confirmed", "expired"]
DEFAULT_PENDING_TTL_SECONDS = 210


@dataclass(frozen=True)
class PendingTransaction:
    network: str
    account: str
    tx_hash: str
    kind: str
    submitted_at: str


@dataclass(frozen=True)
class PendingResolution:
    pending: PendingTransaction
    status: PendingStatus
    transaction: dict | None = None


class PendingTransactionStore:
    """Small public-metadata store; no XDR, signer, seed, or password is persisted."""

    def __init__(self, path: str | Path):
        self.path = Path(path).expanduser()

    def list(self, network: str, account: str | None = None) -> list[PendingTransaction]:
        items = [item for item in self._load() if item.network == network]
        if account is not None:
            items = [item for item in items if item.account == account]
        return items

    def put(self, pending: PendingTransaction) -> None:
        items = [
            item
            for item in self._load()
            if not (
                item.network == pending.network
                and item.account == pending.account
                and item.tx_hash == pending.tx_hash
            )
        ]
        items.append(pending)
        self._save(items)

    def remove(self, network: str, account: str, tx_hash: str) -> None:
        items = [
            item
            for item in self._load()
            if not (
                item.network == network
                and item.account == account
                and item.tx_hash == tx_hash
            )
        ]
        self._save(items)

    def _load(self) -> list[PendingTransaction]:
        if not self.path.exists():
            return []
        try:
            raw = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, ValueError, TypeError) as exc:
            raise TransactionError(
                "Unable to read pending transaction state",
                details=f"{self.path}: {type(exc).__name__}",
            ) from exc
        if not isinstance(raw, list):
            raise TransactionError(
                "Pending transaction state is malformed",
                details=f"{self.path}: expected a JSON array",
            )

        items = []
        for value in raw:
            if not isinstance(value, dict):
                raise TransactionError(
                    "Pending transaction state is malformed",
                    details=f"{self.path}: expected an object entry",
                )
            try:
                network = value["network"]
                account = value["account"]
                tx_hash = value["tx_hash"]
                submitted_at = value["submitted_at"]
                kind = value.get("kind", "transaction")
            except KeyError as exc:
                raise TransactionError(
                    "Pending transaction state is malformed",
                    details=f"{self.path}: missing {exc.args[0]}",
                ) from exc
            fields = {
                "network": network,
                "account": account,
                "tx_hash": tx_hash,
                "kind": kind,
                "submitted_at": submitted_at,
            }
            if any(not isinstance(item, str) or not item.strip() for item in fields.values()):
                raise TransactionError(
                    "Pending transaction state is malformed",
                    details=f"{self.path}: fields must be non-empty strings",
                )
            items.append(
                PendingTransaction(
                    network=network,
                    account=account,
                    tx_hash=tx_hash,
                    kind=kind,
                    submitted_at=submitted_at,
                )
            )
        return items

    def _save(self, items: list[PendingTransaction]) -> None:
        temporary = self.path.with_suffix(self.path.suffix + ".tmp")
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary.write_text(
                json.dumps([asdict(item) for item in items], indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            temporary.replace(self.path)
        except OSError as exc:
            raise TransactionError(
                "Unable to persist pending transaction state",
                details=f"{self.path}: {type(exc).__name__}",
            ) from exc


class PendingTransactionService:
    def __init__(
        self,
        lookup,
        store: PendingTransactionStore,
        network: str,
        ttl_seconds: int = DEFAULT_PENDING_TTL_SECONDS,
    ):
        self.lookup = lookup
        self.store = store
        self.network = network
        self.ttl_seconds = ttl_seconds

    def remember(self, account: str, tx_hash: str, kind: str = "transaction") -> PendingTransaction:
        pending = PendingTransaction(
            network=self.network,
            account=account,
            tx_hash=tx_hash,
            kind=kind,
            submitted_at=_now(),
        )
        self.store.put(pending)
        return pending

    def has_pending(self, account: str) -> bool:
        """Local-only check used by event-loop UI paths."""
        return bool(self.store.list(self.network, account))

    def first_pending(self, account: str) -> PendingTransaction | None:
        items = self.store.list(self.network, account)
        return items[0] if items else None

    def ensure_clear(self, account: str) -> list[PendingResolution]:
        """Non-blocking local guard for UI event handlers."""
        pending = self.first_pending(account)
        if pending is not None:
            raise TransactionPendingError(pending.tx_hash)
        return []

    def resolve(self, account: str) -> list[PendingResolution]:
        resolutions = []
        now = datetime.now(timezone.utc)
        for pending in self.store.list(self.network, account):
            transaction = self.lookup(pending.tx_hash)
            if transaction is not None:
                self.store.remove(self.network, account, pending.tx_hash)
                resolutions.append(
                    PendingResolution(pending, "confirmed", transaction=transaction)
                )
                continue
            if _age_seconds(pending, now) >= self.ttl_seconds:
                self.store.remove(self.network, account, pending.tx_hash)
                resolutions.append(PendingResolution(pending, "expired"))
            else:
                resolutions.append(PendingResolution(pending, "pending"))
        return resolutions

    def reconcile_and_ensure_clear(self, account: str) -> list[PendingResolution]:
        """Blocking reconciliation for CLI/write orchestration outside an event loop."""
        resolutions = self.resolve(account)
        for item in resolutions:
            if item.status == "pending":
                raise TransactionPendingError(item.pending.tx_hash)
        return resolutions


def _age_seconds(pending: PendingTransaction, now: datetime) -> float:
    try:
        submitted = datetime.fromisoformat(pending.submitted_at.replace("Z", "+00:00"))
    except ValueError:
        return float("inf")
    if submitted.tzinfo is None:
        submitted = submitted.replace(tzinfo=timezone.utc)
    return max(0.0, (now - submitted).total_seconds())


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()

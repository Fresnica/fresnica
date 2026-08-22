"""Local cache for chain-derived data.

Wallet identity and secrets are deliberately not stored here. Every cache key
includes the Stellar network because the same G-address can exist on multiple
networks with different ledger state.
"""

from abc import ABC, abstractmethod
from datetime import datetime, timezone
import json
from pathlib import Path
import sqlite3


class DataStore(ABC):
    @abstractmethod
    def save_balances(self, network: str, account: str, balances: list[dict]) -> None:
        raise NotImplementedError

    @abstractmethod
    def get_balances(self, network: str, account: str) -> list[dict]:
        raise NotImplementedError

    @abstractmethod
    def save_operations(self, network: str, account: str, operations) -> None:
        raise NotImplementedError

    @abstractmethod
    def get_operations(
        self, network: str, account: str, limit: int = 20
    ) -> list[dict]:
        raise NotImplementedError


class MemoryDataStore(DataStore):
    def __init__(self):
        self._balances: dict[tuple[str, str], list[dict]] = {}
        self._operations: dict[tuple[str, str], list[dict]] = {}

    def save_balances(self, network: str, account: str, balances: list[dict]) -> None:
        self._balances[(network, account)] = list(balances)

    def get_balances(self, network: str, account: str) -> list[dict]:
        return list(self._balances.get((network, account), []))

    def save_operations(self, network: str, account: str, operations) -> None:
        self._operations[(network, account)] = _records(operations)

    def get_operations(
        self, network: str, account: str, limit: int = 20
    ) -> list[dict]:
        return list(self._operations.get((network, account), []))[:limit]


class SQLiteDataStore(DataStore):
    """SQLite-backed cache for raw Horizon data used by wallet services."""

    def __init__(self, path: str | Path):
        self.path = Path(path).expanduser()
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._initialize()

    def _connect(self):
        connection = sqlite3.connect(self.path)
        connection.row_factory = sqlite3.Row
        return connection

    def _initialize(self) -> None:
        with self._connect() as db:
            db.executescript(
                """
                PRAGMA journal_mode=WAL;
                CREATE TABLE IF NOT EXISTS balances (
                    network TEXT NOT NULL,
                    account TEXT NOT NULL,
                    asset_key TEXT NOT NULL,
                    raw_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY (network, account, asset_key)
                );
                CREATE INDEX IF NOT EXISTS balances_account_idx
                    ON balances(network, account);

                CREATE TABLE IF NOT EXISTS operations (
                    network TEXT NOT NULL,
                    account TEXT NOT NULL,
                    paging_token TEXT NOT NULL,
                    operation_type TEXT,
                    created_at TEXT,
                    raw_json TEXT NOT NULL,
                    cached_at TEXT NOT NULL,
                    PRIMARY KEY (network, account, paging_token)
                );
                CREATE INDEX IF NOT EXISTS operations_account_idx
                    ON operations(network, account);
                """
            )

    def save_balances(self, network: str, account: str, balances: list[dict]) -> None:
        now = _now()
        with self._connect() as db:
            db.execute(
                "DELETE FROM balances WHERE network = ? AND account = ?",
                (network, account),
            )
            db.executemany(
                """
                INSERT INTO balances(network, account, asset_key, raw_json, updated_at)
                VALUES (?, ?, ?, ?, ?)
                """,
                [
                    (
                        network,
                        account,
                        _asset_key(item),
                        json.dumps(item, separators=(",", ":")),
                        now,
                    )
                    for item in balances
                ],
            )

    def get_balances(self, network: str, account: str) -> list[dict]:
        with self._connect() as db:
            rows = db.execute(
                """
                SELECT raw_json FROM balances
                WHERE network = ? AND account = ?
                ORDER BY CASE WHEN asset_key = 'native' THEN 0 ELSE 1 END, asset_key
                """,
                (network, account),
            ).fetchall()
        return [json.loads(row["raw_json"]) for row in rows]

    def save_operations(self, network: str, account: str, operations) -> None:
        now = _now()
        records = _records(operations)
        with self._connect() as db:
            db.executemany(
                """
                INSERT OR REPLACE INTO operations(
                    network, account, paging_token, operation_type,
                    created_at, raw_json, cached_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
                [
                    (
                        network,
                        account,
                        str(item.get("paging_token", item.get("id", ""))),
                        item.get("type"),
                        item.get("created_at"),
                        json.dumps(item, separators=(",", ":")),
                        now,
                    )
                    for item in records
                    if item.get("paging_token") is not None or item.get("id") is not None
                ],
            )

    def get_operations(
        self, network: str, account: str, limit: int = 20
    ) -> list[dict]:
        with self._connect() as db:
            rows = db.execute(
                """
                SELECT raw_json FROM operations
                WHERE network = ? AND account = ?
                ORDER BY CAST(paging_token AS INTEGER) DESC
                LIMIT ?
                """,
                (network, account, limit),
            ).fetchall()
        return [json.loads(row["raw_json"]) for row in rows]


def _records(payload) -> list[dict]:
    if isinstance(payload, list):
        return list(payload)
    if isinstance(payload, dict):
        return list(payload.get("_embedded", {}).get("records", []))
    return []


def _asset_key(balance: dict) -> str:
    if balance.get("asset_type") == "native":
        return "native"
    return f"{balance.get('asset_code', '')}:{balance.get('asset_issuer', '')}"


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()

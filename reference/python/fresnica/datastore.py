"""Local cache for chain-derived data.

Wallet identity and secrets are deliberately not stored here. Every cache key
includes the Stellar network because the same G-address and asset identifiers
can exist on multiple networks with different ledger state.
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

    @abstractmethod
    def save_liquidity_pool(self, network: str, pool_id: str, pool: dict) -> None:
        raise NotImplementedError

    @abstractmethod
    def get_liquidity_pool(self, network: str, pool_id: str) -> dict | None:
        raise NotImplementedError

    @abstractmethod
    def save_offers(self, network: str, account: str, offers) -> None:
        raise NotImplementedError

    @abstractmethod
    def get_offers(self, network: str, account: str, limit: int = 20) -> list[dict]:
        raise NotImplementedError

    @abstractmethod
    def save_trades(self, network: str, pair_key: str, trades) -> None:
        raise NotImplementedError

    @abstractmethod
    def get_trades(self, network: str, pair_key: str, limit: int = 20) -> list[dict]:
        raise NotImplementedError

    @abstractmethod
    def save_trade_aggregations(
        self, network: str, pair_key: str, resolution: int, aggregations
    ) -> None:
        raise NotImplementedError

    @abstractmethod
    def get_trade_aggregations(
        self, network: str, pair_key: str, resolution: int, limit: int = 100
    ) -> list[dict]:
        raise NotImplementedError


class MemoryDataStore(DataStore):
    def __init__(self):
        self._balances: dict[tuple[str, str], list[dict]] = {}
        self._operations: dict[tuple[str, str], dict[str, dict]] = {}
        self._liquidity_pools: dict[tuple[str, str], dict] = {}
        self._offers: dict[tuple[str, str], list[dict]] = {}
        self._trades: dict[tuple[str, str], dict[str, dict]] = {}
        self._trade_aggregations: dict[tuple[str, str, int], dict[str, dict]] = {}

    def save_balances(self, network: str, account: str, balances: list[dict]) -> None:
        self._balances[(network, account)] = list(balances)

    def get_balances(self, network: str, account: str) -> list[dict]:
        return list(self._balances.get((network, account), []))

    def save_operations(self, network: str, account: str, operations) -> None:
        bucket = self._operations.setdefault((network, account), {})
        for item in _records(operations):
            token = str(item.get("paging_token", item.get("id", "")))
            if token:
                bucket[token] = item

    def get_operations(
        self, network: str, account: str, limit: int = 20
    ) -> list[dict]:
        items = list(self._operations.get((network, account), {}).values())
        items.sort(
            key=lambda item: int(item.get("paging_token", item.get("id", 0)) or 0),
            reverse=True,
        )
        return items[:limit]

    def save_liquidity_pool(self, network: str, pool_id: str, pool: dict) -> None:
        self._liquidity_pools[(network, pool_id)] = dict(pool)

    def get_liquidity_pool(self, network: str, pool_id: str) -> dict | None:
        pool = self._liquidity_pools.get((network, pool_id))
        return dict(pool) if pool is not None else None

    def save_offers(self, network: str, account: str, offers) -> None:
        self._offers[(network, account)] = _records(offers)

    def get_offers(self, network: str, account: str, limit: int = 20) -> list[dict]:
        return list(self._offers.get((network, account), []))[:limit]

    def save_trades(self, network: str, pair_key: str, trades) -> None:
        bucket = self._trades.setdefault((network, pair_key), {})
        for item in _records(trades):
            token = str(item.get("paging_token", item.get("id", "")))
            if token:
                bucket[token] = item

    def get_trades(self, network: str, pair_key: str, limit: int = 20) -> list[dict]:
        items = list(self._trades.get((network, pair_key), {}).values())
        items.sort(
            key=lambda item: (
                item.get("ledger_close_time", ""),
                str(item.get("paging_token", "")),
            ),
            reverse=True,
        )
        return items[:limit]

    def save_trade_aggregations(
        self, network: str, pair_key: str, resolution: int, aggregations
    ) -> None:
        bucket = self._trade_aggregations.setdefault((network, pair_key, resolution), {})
        for item in _records(aggregations):
            timestamp = str(item.get("timestamp", ""))
            if timestamp:
                bucket[timestamp] = item

    def get_trade_aggregations(
        self, network: str, pair_key: str, resolution: int, limit: int = 100
    ) -> list[dict]:
        items = list(self._trade_aggregations.get((network, pair_key, resolution), {}).values())
        items.sort(key=lambda item: int(item.get("timestamp", 0)), reverse=True)
        return items[:limit]


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

                CREATE TABLE IF NOT EXISTS liquidity_pools (
                    network TEXT NOT NULL,
                    pool_id TEXT NOT NULL,
                    raw_json TEXT NOT NULL,
                    cached_at TEXT NOT NULL,
                    PRIMARY KEY (network, pool_id)
                );

                CREATE TABLE IF NOT EXISTS offers (
                    network TEXT NOT NULL,
                    account TEXT NOT NULL,
                    offer_id TEXT NOT NULL,
                    seller TEXT,
                    selling_key TEXT,
                    buying_key TEXT,
                    amount TEXT,
                    price TEXT,
                    last_modified_ledger INTEGER,
                    raw_json TEXT NOT NULL,
                    cached_at TEXT NOT NULL,
                    PRIMARY KEY (network, account, offer_id)
                );
                CREATE INDEX IF NOT EXISTS offers_account_idx
                    ON offers(network, account);

                CREATE TABLE IF NOT EXISTS trades (
                    network TEXT NOT NULL,
                    pair_key TEXT NOT NULL,
                    paging_token TEXT NOT NULL,
                    ledger_close_time TEXT,
                    base_amount TEXT,
                    counter_amount TEXT,
                    base_is_seller INTEGER,
                    raw_json TEXT NOT NULL,
                    cached_at TEXT NOT NULL,
                    PRIMARY KEY (network, pair_key, paging_token)
                );
                CREATE INDEX IF NOT EXISTS trades_pair_idx
                    ON trades(network, pair_key, ledger_close_time);

                CREATE TABLE IF NOT EXISTS trade_aggregations (
                    network TEXT NOT NULL,
                    pair_key TEXT NOT NULL,
                    resolution INTEGER NOT NULL,
                    timestamp TEXT NOT NULL,
                    trade_count INTEGER,
                    base_volume TEXT,
                    counter_volume TEXT,
                    open TEXT,
                    high TEXT,
                    low TEXT,
                    close TEXT,
                    avg TEXT,
                    raw_json TEXT NOT NULL,
                    cached_at TEXT NOT NULL,
                    PRIMARY KEY (network, pair_key, resolution, timestamp)
                );
                CREATE INDEX IF NOT EXISTS trade_aggregations_pair_idx
                    ON trade_aggregations(network, pair_key, resolution, timestamp);
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

    def save_liquidity_pool(self, network: str, pool_id: str, pool: dict) -> None:
        with self._connect() as db:
            db.execute(
                """
                INSERT OR REPLACE INTO liquidity_pools(network, pool_id, raw_json, cached_at)
                VALUES (?, ?, ?, ?)
                """,
                (
                    network,
                    pool_id,
                    json.dumps(pool, separators=(",", ":")),
                    _now(),
                ),
            )

    def get_liquidity_pool(self, network: str, pool_id: str) -> dict | None:
        with self._connect() as db:
            row = db.execute(
                """
                SELECT raw_json FROM liquidity_pools
                WHERE network = ? AND pool_id = ?
                """,
                (network, pool_id),
            ).fetchone()
        return json.loads(row["raw_json"]) if row is not None else None

    def save_offers(self, network: str, account: str, offers) -> None:
        now = _now()
        records = _records(offers)
        with self._connect() as db:
            db.execute(
                "DELETE FROM offers WHERE network = ? AND account = ?",
                (network, account),
            )
            db.executemany(
                """
                INSERT INTO offers(
                    network, account, offer_id, seller, selling_key, buying_key,
                    amount, price, last_modified_ledger, raw_json, cached_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                [
                    (
                        network,
                        account,
                        str(item.get("id", item.get("paging_token", ""))),
                        item.get("seller"),
                        _asset_key(item.get("selling", {})),
                        _asset_key(item.get("buying", {})),
                        item.get("amount"),
                        item.get("price"),
                        item.get("last_modified_ledger"),
                        json.dumps(item, separators=(",", ":")),
                        now,
                    )
                    for item in records
                    if item.get("id") is not None or item.get("paging_token") is not None
                ],
            )

    def get_offers(self, network: str, account: str, limit: int = 20) -> list[dict]:
        with self._connect() as db:
            rows = db.execute(
                """
                SELECT raw_json FROM offers
                WHERE network = ? AND account = ?
                ORDER BY CAST(offer_id AS INTEGER) DESC
                LIMIT ?
                """,
                (network, account, limit),
            ).fetchall()
        return [json.loads(row["raw_json"]) for row in rows]

    def save_trades(self, network: str, pair_key: str, trades) -> None:
        now = _now()
        records = _records(trades)
        with self._connect() as db:
            db.executemany(
                """
                INSERT OR REPLACE INTO trades(
                    network, pair_key, paging_token, ledger_close_time,
                    base_amount, counter_amount, base_is_seller, raw_json, cached_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                [
                    (
                        network,
                        pair_key,
                        str(item.get("paging_token", item.get("id", ""))),
                        item.get("ledger_close_time"),
                        item.get("base_amount"),
                        item.get("counter_amount"),
                        1 if item.get("base_is_seller") else 0,
                        json.dumps(item, separators=(",", ":")),
                        now,
                    )
                    for item in records
                    if item.get("paging_token") is not None or item.get("id") is not None
                ],
            )

    def get_trades(self, network: str, pair_key: str, limit: int = 20) -> list[dict]:
        with self._connect() as db:
            rows = db.execute(
                """
                SELECT raw_json FROM trades
                WHERE network = ? AND pair_key = ?
                ORDER BY ledger_close_time DESC, paging_token DESC
                LIMIT ?
                """,
                (network, pair_key, limit),
            ).fetchall()
        return [json.loads(row["raw_json"]) for row in rows]

    def save_trade_aggregations(
        self, network: str, pair_key: str, resolution: int, aggregations
    ) -> None:
        now = _now()
        records = _records(aggregations)
        with self._connect() as db:
            db.executemany(
                """
                INSERT OR REPLACE INTO trade_aggregations(
                    network, pair_key, resolution, timestamp, trade_count,
                    base_volume, counter_volume, open, high, low, close, avg,
                    raw_json, cached_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                [
                    (
                        network,
                        pair_key,
                        resolution,
                        str(item.get("timestamp", "")),
                        item.get("trade_count"),
                        item.get("base_volume"),
                        item.get("counter_volume"),
                        item.get("open"),
                        item.get("high"),
                        item.get("low"),
                        item.get("close"),
                        item.get("avg"),
                        json.dumps(item, separators=(",", ":")),
                        now,
                    )
                    for item in records
                    if item.get("timestamp") is not None
                ],
            )

    def get_trade_aggregations(
        self, network: str, pair_key: str, resolution: int, limit: int = 100
    ) -> list[dict]:
        with self._connect() as db:
            rows = db.execute(
                """
                SELECT raw_json FROM trade_aggregations
                WHERE network = ? AND pair_key = ? AND resolution = ?
                ORDER BY CAST(timestamp AS INTEGER) DESC
                LIMIT ?
                """,
                (network, pair_key, resolution, limit),
            ).fetchall()
        return [json.loads(row["raw_json"]) for row in rows]


def _records(payload) -> list[dict]:
    if isinstance(payload, list):
        return list(payload)
    if isinstance(payload, dict):
        return list(payload.get("_embedded", {}).get("records", []))
    return []


def _asset_key(asset: dict) -> str:
    asset_type = asset.get("asset_type")
    if asset_type == "native":
        return "native"
    if asset_type == "liquidity_pool_shares":
        return f"pool:{asset.get('liquidity_pool_id', '')}"
    return f"{asset.get('asset_code', '')}:{asset.get('asset_issuer', '')}"


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()

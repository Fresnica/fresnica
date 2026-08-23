from datetime import datetime, timedelta, timezone
import json

import pytest

from fresnica.errors import NetworkError, TransactionPendingError
from fresnica.pending_transactions import (
    PendingTransaction,
    PendingTransactionService,
    PendingTransactionStore,
)


def test_pending_store_persists_public_metadata_only(tmp_path):
    path = tmp_path / "pending.json"
    store = PendingTransactionStore(path)
    service = PendingTransactionService(lambda tx_hash: None, store, "testnet")

    service.remember("GACCOUNT", "abc123", kind="offer:create")

    raw = json.loads(path.read_text(encoding="utf-8"))
    assert raw == [
        {
            "account": "GACCOUNT",
            "kind": "offer:create",
            "network": "testnet",
            "submitted_at": raw[0]["submitted_at"],
            "tx_hash": "abc123",
        }
    ]
    assert "secret" not in path.read_text(encoding="utf-8").lower()
    assert "xdr" not in path.read_text(encoding="utf-8").lower()


def test_ui_local_guard_never_calls_lookup(tmp_path):
    calls = []
    store = PendingTransactionStore(tmp_path / "pending.json")
    service = PendingTransactionService(lambda tx_hash: calls.append(tx_hash), store, "mainnet")
    service.remember("GACCOUNT", "abc123")

    with pytest.raises(TransactionPendingError) as exc:
        service.ensure_clear("GACCOUNT")

    assert exc.value.tx_hash == "abc123"
    assert calls == []


def test_cli_reconciliation_keeps_recent_not_found_transaction_pending(tmp_path):
    store = PendingTransactionStore(tmp_path / "pending.json")
    service = PendingTransactionService(lambda tx_hash: None, store, "testnet", ttl_seconds=210)
    service.remember("GACCOUNT", "abc123")

    with pytest.raises(TransactionPendingError):
        service.reconcile_and_ensure_clear("GACCOUNT")

    assert service.has_pending("GACCOUNT")


def test_reconciliation_removes_transaction_when_horizon_finds_it(tmp_path):
    store = PendingTransactionStore(tmp_path / "pending.json")
    service = PendingTransactionService(
        lambda tx_hash: {"hash": tx_hash, "successful": True},
        store,
        "testnet",
    )
    service.remember("GACCOUNT", "abc123")

    resolutions = service.reconcile_and_ensure_clear("GACCOUNT")

    assert len(resolutions) == 1
    assert resolutions[0].status == "confirmed"
    assert resolutions[0].transaction["hash"] == "abc123"
    assert not service.has_pending("GACCOUNT")


def test_reconciliation_expires_old_transaction_only_after_lookup_misses(tmp_path):
    store = PendingTransactionStore(tmp_path / "pending.json")
    old = (datetime.now(timezone.utc) - timedelta(minutes=10)).isoformat()
    store.put(
        PendingTransaction(
            network="testnet",
            account="GACCOUNT",
            tx_hash="abc123",
            kind="payment",
            submitted_at=old,
        )
    )
    service = PendingTransactionService(lambda tx_hash: None, store, "testnet", ttl_seconds=210)

    resolutions = service.reconcile_and_ensure_clear("GACCOUNT")

    assert resolutions[0].status == "expired"
    assert not service.has_pending("GACCOUNT")


def test_network_failure_keeps_pending_record_for_later_retry(tmp_path):
    store = PendingTransactionStore(tmp_path / "pending.json")

    def fail(_):
        raise NetworkError("offline")

    service = PendingTransactionService(fail, store, "testnet")
    service.remember("GACCOUNT", "abc123")

    with pytest.raises(NetworkError):
        service.reconcile_and_ensure_clear("GACCOUNT")

    assert service.has_pending("GACCOUNT")

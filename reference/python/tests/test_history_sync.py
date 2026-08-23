from stellar_sdk import Keypair

from fresnica.datastore import MemoryDataStore
from fresnica.history_service import (
    HistoryService,
    SYNC_MAX_INCREMENTAL_PAGES,
    SYNC_PAGE_LIMIT,
)
from fresnica.wallet import Wallet


def _record(token):
    return {
        "paging_token": str(token),
        "id": str(token),
        "transaction_hash": f"tx-{token}",
        "type": "manage_data",
        "created_at": "2026-08-23T00:00:00Z",
        "name": f"entry-{token}",
    }


def _page(records):
    return {"_embedded": {"records": records}}


class PagingAdapter:
    def __init__(self, pages):
        self.pages = list(pages)
        self.calls = []

    def get_operations(self, address, limit=200, cursor=None, desc=True):
        self.calls.append((address, limit, cursor, desc))
        if not self.pages:
            return _page([])
        return _page(self.pages.pop(0))


def test_history_incremental_sync_pages_until_horizon_head():
    account = Keypair.random().public_key
    wallet = Wallet.from_address(account)
    store = MemoryDataStore()
    store.save_operations("mainnet", account, [_record(100)])
    adapter = PagingAdapter([
        [_record(token) for token in range(101, 301)],
        [_record(token) for token in range(301, 451)],
    ])
    service = HistoryService(adapter, store, "mainnet")

    result = service.sync_recent(wallet)

    assert result.fetched_count == 350
    assert result.caught_up is True
    assert [call[2] for call in adapter.calls] == ["100", "300"]
    assert all(call[3] is False for call in adapter.calls)
    assert service.cached_operation_count(wallet) == 351


def test_history_incremental_sync_reports_bounded_incomplete_catch_up():
    account = Keypair.random().public_key
    wallet = Wallet.from_address(account)
    store = MemoryDataStore()
    store.save_operations("mainnet", account, [_record(1)])
    pages = []
    start = 2
    for _ in range(SYNC_MAX_INCREMENTAL_PAGES):
        pages.append([_record(token) for token in range(start, start + SYNC_PAGE_LIMIT)])
        start += SYNC_PAGE_LIMIT
    adapter = PagingAdapter(pages)
    service = HistoryService(adapter, store, "mainnet")

    result = service.sync_recent(wallet)

    assert result.fetched_count == SYNC_MAX_INCREMENTAL_PAGES * SYNC_PAGE_LIMIT
    assert result.caught_up is False
    assert len(adapter.calls) == SYNC_MAX_INCREMENTAL_PAGES


def test_history_initial_descending_snapshot_is_at_current_head():
    account = Keypair.random().public_key
    wallet = Wallet.from_address(account)
    store = MemoryDataStore()
    adapter = PagingAdapter([[_record(token) for token in range(400, 200, -1)]])
    service = HistoryService(adapter, store, "mainnet")

    result = service.sync_recent(wallet)

    assert result.fetched_count == 200
    assert result.caught_up is True
    assert adapter.calls[0][2] is None
    assert adapter.calls[0][3] is True

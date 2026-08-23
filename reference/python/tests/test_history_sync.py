from stellar_sdk import Keypair

from fresnica.datastore import MemoryDataStore
from fresnica.history_service import HISTORY_CACHE_LIMIT, HistoryService, SYNC_PAGE_LIMIT
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


def test_empty_history_bootstraps_latest_2000_from_head_backwards():
    account = Keypair.random().public_key
    wallet = Wallet.from_address(account)
    store = MemoryDataStore()
    pages = []
    for high in range(HISTORY_CACHE_LIMIT, 0, -SYNC_PAGE_LIMIT):
        low = max(1, high - SYNC_PAGE_LIMIT + 1)
        pages.append([_record(token) for token in range(high, low - 1, -1)])
    adapter = PagingAdapter(pages)
    service = HistoryService(adapter, store, "mainnet")

    fetched = service.sync_recent(wallet)

    assert fetched == HISTORY_CACHE_LIMIT
    assert len(adapter.calls) == HISTORY_CACHE_LIMIT // SYNC_PAGE_LIMIT
    assert adapter.calls[0][2:] == (None, True)
    assert adapter.calls[1][2:] == (str(HISTORY_CACHE_LIMIT - SYNC_PAGE_LIMIT + 1), True)
    cached = store.get_operations("mainnet", account, limit=None)
    assert len(cached) == HISTORY_CACHE_LIMIT
    assert cached[0]["paging_token"] == str(HISTORY_CACHE_LIMIT)
    assert cached[-1]["paging_token"] == "1"


def test_existing_history_walks_old_to_new_without_page_cap_and_trims_oldest():
    account = Keypair.random().public_key
    wallet = Wallet.from_address(account)
    store = MemoryDataStore()
    store.save_operations("mainnet", account, [_record(token) for token in range(1, 2001)])
    pages = [
        [_record(token) for token in range(start, start + SYNC_PAGE_LIMIT)]
        for start in range(2001, 4401, SYNC_PAGE_LIMIT)
    ]
    pages.append([])
    adapter = PagingAdapter(pages)
    service = HistoryService(adapter, store, "mainnet")

    fetched = service.sync_recent(wallet)

    assert fetched == 2400
    assert len(adapter.calls) == 13
    assert all(call[3] is False for call in adapter.calls)
    assert adapter.calls[0][2] == "2000"
    cached = store.get_operations("mainnet", account, limit=None)
    assert len(cached) == HISTORY_CACHE_LIMIT
    assert cached[0]["paging_token"] == "4400"
    assert cached[-1]["paging_token"] == "2401"


def test_full_history_bootstrap_keeps_every_available_operation():
    account = Keypair.random().public_key
    wallet = Wallet.from_address(account)
    store = MemoryDataStore()
    pages = [
        [_record(token) for token in range(650, 450, -1)],
        [_record(token) for token in range(450, 250, -1)],
        [_record(token) for token in range(250, 50, -1)],
        [_record(token) for token in range(50, 0, -1)],
    ]
    adapter = PagingAdapter(pages)
    service = HistoryService(adapter, store, "mainnet", keep_full_history=True)

    fetched = service.sync_recent(wallet)

    assert fetched == 650
    assert service.cached_operation_count(wallet) == 650
    assert all(call[3] is True for call in adapter.calls)


def test_enabling_full_history_catches_up_then_backfills_older_records():
    account = Keypair.random().public_key
    wallet = Wallet.from_address(account)
    store = MemoryDataStore()
    store.save_operations("mainnet", account, [_record(token) for token in range(101, 2101)])
    adapter = PagingAdapter([
        [_record(2101), _record(2102)],
        [_record(token) for token in range(100, 0, -1)],
    ])
    service = HistoryService(adapter, store, "mainnet", keep_full_history=True)

    fetched = service.sync_recent(wallet)

    assert fetched == 102
    assert [call[3] for call in adapter.calls] == [False, True]
    assert [call[2] for call in adapter.calls] == ["2100", "101"]
    assert service.cached_operation_count(wallet) == 2102


def test_default_mode_does_not_network_backfill_older_history():
    account = Keypair.random().public_key
    wallet = Wallet.from_address(account)
    store = MemoryDataStore()
    store.save_operations("mainnet", account, [_record(10)])
    adapter = PagingAdapter([])
    service = HistoryService(adapter, store, "mainnet")

    assert service.load_older(wallet) == 0
    assert adapter.calls == []

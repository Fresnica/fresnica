from stellar_sdk import Keypair

from fresnica.datastore import MemoryDataStore
from fresnica.history_service import HistoryService
from fresnica.wallet import Wallet


class NoNetworkAdapter:
    def get_operations(self, *args, **kwargs):
        raise AssertionError("cached activity test must not hit the network")


def test_activity_groups_operations_by_transaction_hash():
    account = Keypair.random().public_key
    destination = Keypair.random().public_key
    store = MemoryDataStore()
    store.save_operations(
        "mainnet",
        account,
        [
            {
                "paging_token": "30",
                "transaction_hash": "tx-one",
                "type": "payment",
                "created_at": "2026-08-22T12:00:00Z",
                "from": account,
                "to": destination,
                "asset_type": "native",
                "amount": "1.0000000",
            },
            {
                "paging_token": "29",
                "transaction_hash": "tx-one",
                "type": "manage_data",
                "created_at": "2026-08-22T12:00:00Z",
                "source_account": account,
                "name": "profile",
            },
            {
                "paging_token": "20",
                "transaction_hash": "tx-two",
                "type": "set_options",
                "created_at": "2026-08-22T11:00:00Z",
                "source_account": account,
            },
        ],
    )

    service = HistoryService(NoNetworkAdapter(), store, "mainnet")
    activities = service.get_activity_views(
        Wallet.from_address(account),
        limit=20,
        refresh=False,
    )

    assert len(activities) == 2
    assert activities[0].transaction_hash == "tx-one"
    assert activities[0].operation_count == 2
    assert "Sent 1 XLM" in activities[0].summary
    assert "Updated account data: profile" in activities[0].summary
    assert activities[1].transaction_hash == "tx-two"
    assert activities[1].summary == "Updated account settings"

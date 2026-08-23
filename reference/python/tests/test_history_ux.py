from stellar_sdk import Keypair

from fresnica.datastore import MemoryDataStore
from fresnica.history_service import (
    HistoryService,
    activity_counterparties,
    activity_summary_for_display,
    is_suspicious_claimable_activity,
)
from fresnica.wallet import Wallet


class NoNetworkAdapter:
    def get_operations(self, *args, **kwargs):
        raise AssertionError("cached history test must not hit the network")


def _service(account, records):
    store = MemoryDataStore()
    store.save_operations("mainnet", account, records)
    return HistoryService(NoNetworkAdapter(), store, "mainnet")


def test_manage_buy_offer_uses_horizon_amount_field():
    account = Keypair.random().public_key
    issuer = Keypair.random().public_key
    service = _service(
        account,
        [
            {
                "paging_token": "10",
                "type": "manage_buy_offer",
                "source_account": account,
                "amount": "20.4521401",
                "offer_id": "0",
                "price": "0.0300003",
                "buying_asset_type": "native",
                "selling_asset_type": "credit_alphanum4",
                "selling_asset_code": "EURT",
                "selling_asset_issuer": issuer,
            }
        ],
    )

    [view] = service.get_views(Wallet.from_address(account), refresh=False)
    assert "Placed BUY 20.4521401 XLM" in view.summary


def test_standalone_incoming_claimable_is_suspicious_but_outgoing_is_not():
    account = Keypair.random().public_key
    sender = Keypair.random().public_key
    recipient = Keypair.random().public_key
    asset_issuer = Keypair.random().public_key
    service = _service(
        account,
        [
            {
                "paging_token": "20",
                "transaction_hash": "incoming",
                "type": "create_claimable_balance",
                "source_account": sender,
                "amount": "0.0000001",
                "asset": f"SPAM:{asset_issuer}",
                "claimants": [{"destination": account, "predicate": {"unconditional": True}}],
            },
            {
                "paging_token": "19",
                "transaction_hash": "outgoing",
                "type": "create_claimable_balance",
                "source_account": account,
                "amount": "5.0000000",
                "asset": "native",
                "claimants": [{"destination": recipient, "predicate": {"unconditional": True}}],
            },
        ],
    )

    activities = service.get_activity_views(Wallet.from_address(account), refresh=False)
    assert "Incoming claimable asset" in activities[0].summary
    assert "review before claiming" in activities[0].summary
    assert is_suspicious_claimable_activity(activities[0])
    assert "Created claimable payment: 5 XLM" in activities[1].summary
    assert not is_suspicious_claimable_activity(activities[1])
    assert sender in activity_counterparties(activities[0], account)
    assert recipient in activity_counterparties(activities[1], account)


def test_claimable_plus_clawback_transaction_is_never_hidden_as_suspicious():
    """Aqua-style mixed transactions must not disappear with spam filtering."""
    account = Keypair.random().public_key
    sender = Keypair.random().public_key
    issuer = Keypair.random().public_key
    service = _service(
        account,
        [
            {
                "paging_token": "31",
                "transaction_hash": "mixed",
                "type": "create_claimable_balance",
                "source_account": sender,
                "amount": "0.0000001",
                "asset": f"AQUA:{issuer}",
                "claimants": [{"destination": account, "predicate": {"unconditional": True}}],
            },
            {
                "paging_token": "30",
                "transaction_hash": "mixed",
                "type": "clawback",
                "source_account": issuer,
                "from": account,
                "amount": "100.0000000",
                "asset_type": "credit_alphanum4",
                "asset_code": "AQUA",
                "asset_issuer": issuer,
            },
        ],
    )

    [activity] = service.get_activity_views(Wallet.from_address(account), refresh=False)

    assert activity.operation_count == 2
    assert "Clawback" in activity.summary
    assert not is_suspicious_claimable_activity(activity)


def test_contract_call_summarizes_asset_changes_and_uses_current_metadata():
    account = Keypair.random().public_key
    sender = Keypair.random().public_key
    issuer = Keypair.random().public_key
    service = _service(
        account,
        [
            {
                "paging_token": "40",
                "transaction_hash": "contract",
                "type": "invoke_host_function",
                "source_account": account,
                "function": "HostFunctionTypeHostFunctionTypeInvokeContract",
                "asset_balance_changes": [
                    {
                        "asset_type": "credit_alphanum4",
                        "asset_code": "USDC",
                        "asset_issuer": issuer,
                        "type": "transfer",
                        "from": sender,
                        "to": account,
                        "amount": "12.5000000",
                    }
                ],
            }
        ],
    )

    [activity] = service.get_activity_views(Wallet.from_address(account), refresh=False)
    summary = activity_summary_for_display(
        activity,
        account,
        contact_names={sender: "Alice"},
        issuer_domains={issuer: "circle.com"},
    )

    assert summary.startswith("Contract call · Received 12.5 USDC · circle.com")
    assert "Alice ·" in summary
    assert sender in activity_counterparties(activity, account)


def test_loading_older_records_expands_activity_window_beyond_200_operations():
    account = Keypair.random().public_key
    records = []
    for token in range(1, 451):
        records.append(
            {
                "paging_token": str(token),
                "transaction_hash": f"tx-{token}",
                "type": "manage_data",
                "source_account": account,
                "name": f"entry-{token}",
            }
        )
    service = _service(account, records)

    assert service.cached_operation_count(Wallet.from_address(account)) == 450
    activities = service.get_activity_views(
        Wallet.from_address(account),
        limit=450,
        refresh=False,
    )
    assert len(activities) == 450
    assert "entry-1" in activities[-1].summary

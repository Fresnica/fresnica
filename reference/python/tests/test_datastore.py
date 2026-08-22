from fresnica.datastore import SQLiteDataStore


def test_sqlite_balances_are_scoped_by_network(tmp_path):
    store = SQLiteDataStore(tmp_path / "chain.sqlite3")
    address = "G-SAME-IDENTITY"
    store.save_balances("mainnet", address, [{"asset_type": "native", "balance": "10"}])
    store.save_balances("testnet", address, [{"asset_type": "native", "balance": "99"}])

    assert store.get_balances("mainnet", address)[0]["balance"] == "10"
    assert store.get_balances("testnet", address)[0]["balance"] == "99"


def test_sqlite_balance_snapshot_removes_old_assets(tmp_path):
    store = SQLiteDataStore(tmp_path / "chain.sqlite3")
    address = "G"
    store.save_balances(
        "mainnet",
        address,
        [
            {"asset_type": "native", "balance": "10"},
            {
                "asset_type": "credit_alphanum4",
                "asset_code": "USD",
                "asset_issuer": "GI",
                "balance": "5",
            },
        ],
    )
    store.save_balances("mainnet", address, [{"asset_type": "native", "balance": "9"}])

    balances = store.get_balances("mainnet", address)
    assert len(balances) == 1
    assert balances[0]["balance"] == "9"


def test_sqlite_operations_keep_raw_horizon_records(tmp_path):
    store = SQLiteDataStore(tmp_path / "chain.sqlite3")
    payload = {
        "_embedded": {
            "records": [
                {
                    "paging_token": "10",
                    "type": "payment",
                    "created_at": "2026-01-01T00:00:00Z",
                },
                {
                    "paging_token": "11",
                    "type": "manage_sell_offer",
                    "created_at": "2026-01-01T00:00:01Z",
                },
            ]
        }
    }
    store.save_operations("mainnet", "G", payload)

    records = store.get_operations("mainnet", "G")
    assert [item["paging_token"] for item in records] == ["11", "10"]

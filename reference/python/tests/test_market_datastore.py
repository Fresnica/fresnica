from fresnica.datastore import MemoryDataStore, SQLiteDataStore


OFFER = {
    "id": "7",
    "paging_token": "7",
    "seller": "GSELLER",
    "selling": {"asset_type": "native"},
    "buying": {
        "asset_type": "credit_alphanum4",
        "asset_code": "USD",
        "asset_issuer": "GISSUER",
    },
    "amount": "12.5",
    "price": "0.25",
    "last_modified_ledger": 123,
}

TRADE = {
    "id": "99-0",
    "paging_token": "99-0",
    "ledger_close_time": "2026-08-22T12:00:00Z",
    "base_amount": "2",
    "counter_amount": "5",
    "base_is_seller": True,
}

AGGREGATION = {
    "timestamp": "1787400000000",
    "trade_count": 3,
    "base_volume": "6",
    "counter_volume": "15",
    "open": "2.4",
    "high": "2.6",
    "low": "2.3",
    "close": "2.5",
    "avg": "2.5",
}


def _exercise(store):
    store.save_offers("mainnet", "GACCOUNT", {"_embedded": {"records": [OFFER]}})
    store.save_offers("testnet", "GACCOUNT", {"_embedded": {"records": []}})
    assert store.get_offers("mainnet", "GACCOUNT") == [OFFER]
    assert store.get_offers("testnet", "GACCOUNT") == []

    store.save_trades("mainnet", "XLM>USD:GISSUER", {"_embedded": {"records": [TRADE]}})
    store.save_trades("testnet", "XLM>USD:GISSUER", {"_embedded": {"records": []}})
    assert store.get_trades("mainnet", "XLM>USD:GISSUER") == [TRADE]
    assert store.get_trades("testnet", "XLM>USD:GISSUER") == []

    store.save_trade_aggregations(
        "mainnet",
        "XLM>USD:GISSUER",
        3_600_000,
        {"_embedded": {"records": [AGGREGATION]}},
    )
    assert store.get_trade_aggregations(
        "mainnet", "XLM>USD:GISSUER", 3_600_000
    ) == [AGGREGATION]
    assert store.get_trade_aggregations(
        "mainnet", "XLM>USD:GISSUER", 60_000
    ) == []


def _exercise_trade_cursor_order(store):
    same_time = "2026-08-23T00:00:00Z"
    records = [
        {**TRADE, "id": "100089067462524929-2", "paging_token": "100089067462524929-2", "ledger_close_time": same_time},
        {**TRADE, "id": "100089067462524929-10", "paging_token": "100089067462524929-10", "ledger_close_time": same_time},
        {**TRADE, "id": "100089067462524930-0", "paging_token": "100089067462524930-0", "ledger_close_time": same_time},
    ]
    store.save_trades("mainnet", "account:GACCOUNT", records)
    assert [
        item["paging_token"]
        for item in store.get_trades("mainnet", "account:GACCOUNT", limit=3)
    ] == [
        "100089067462524930-0",
        "100089067462524929-10",
        "100089067462524929-2",
    ]


def test_memory_market_cache():
    store = MemoryDataStore()
    _exercise(store)
    _exercise_trade_cursor_order(store)


def test_sqlite_market_cache(tmp_path):
    store = SQLiteDataStore(tmp_path / "market.sqlite3")
    _exercise(store)
    _exercise_trade_cursor_order(store)

    with store._connect() as db:
        offer = db.execute(
            "SELECT selling_key, buying_key, amount, price FROM offers WHERE offer_id = '7'"
        ).fetchone()
        assert tuple(offer) == ("native", "USD:GISSUER", "12.5", "0.25")

        trade = db.execute(
            "SELECT base_amount, counter_amount, base_is_seller FROM trades WHERE paging_token = '99-0'"
        ).fetchone()
        assert tuple(trade) == ("2", "5", 1)

        candle = db.execute(
            "SELECT trade_count, open, high, low, close FROM trade_aggregations"
        ).fetchone()
        assert tuple(candle) == (3, "2.4", "2.6", "2.3", "2.5")

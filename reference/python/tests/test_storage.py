from fresnica.storage import FileWalletStorage, WalletRecord


def test_file_wallet_storage_supports_unicode_names_and_default(tmp_path):
    store = FileWalletStorage(tmp_path / "wallets")
    record = WalletRecord(
        name="主钱包",
        address="GADDRESS",
        wallet_type="watch-only",
        network="mainnet",
    )
    store.save(record)
    store.set_default(record.name)

    loaded = store.load("主钱包")
    assert loaded.address == "GADDRESS"
    assert store.get_default() == "主钱包"
    assert store.list()[0].name == "主钱包"

from fresnica.settings import SettingsStore, UserSettings


def test_settings_store_persists_zero_balance_preference(tmp_path):
    path = tmp_path / "settings.json"
    store = SettingsStore(path)

    assert store.load().show_zero_balances is False

    store.save(UserSettings(show_zero_balances=True))
    assert store.load().show_zero_balances is True


def test_settings_store_recovers_from_invalid_json(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text("not-json", encoding="utf-8")
    assert SettingsStore(path).load() == UserSettings()

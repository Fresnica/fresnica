from fresnica.settings import SettingsStore, UserSettings


def test_settings_store_persists_tui_preferences(tmp_path):
    path = tmp_path / "settings.json"
    store = SettingsStore(path)

    assert store.load() == UserSettings()

    store.save(
        UserSettings(
            show_zero_balances=True,
            show_dust_activity=True,
            use_local_time=False,
            theme="nord",
        )
    )
    loaded = store.load()
    assert loaded.show_zero_balances is True
    assert loaded.show_dust_activity is True
    assert loaded.use_local_time is False
    assert loaded.theme == "nord"


def test_settings_store_loads_older_settings_without_new_fields(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text('{"show_zero_balances": true}\n', encoding="utf-8")
    assert SettingsStore(path).load() == UserSettings(show_zero_balances=True)


def test_settings_store_recovers_from_invalid_json(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text("not-json", encoding="utf-8")
    assert SettingsStore(path).load() == UserSettings()

from fresnica.settings import SettingsStore, UserSettings


def test_settings_store_persists_tui_preferences(tmp_path):
    path = tmp_path / "settings.json"
    store = SettingsStore(path)

    assert store.load() == UserSettings()

    store.save(
        UserSettings(
            show_zero_balances=True,
            hide_suspicious_claimables=True,
            use_local_time=False,
            keep_full_history=True,
            theme="nord",
        )
    )
    loaded = store.load()
    assert loaded.show_zero_balances is True
    assert loaded.hide_suspicious_claimables is True
    assert loaded.use_local_time is False
    assert loaded.keep_full_history is True
    assert loaded.theme == "nord"


def test_settings_store_loads_older_settings_without_new_fields(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text('{"show_zero_balances": true}\n', encoding="utf-8")
    assert SettingsStore(path).load() == UserSettings(show_zero_balances=True)


def test_old_dust_setting_does_not_hide_suspicious_activity_by_default(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text('{"show_dust_activity": false}\n', encoding="utf-8")

    loaded = SettingsStore(path).load()

    assert loaded.hide_suspicious_claimables is False


def test_settings_store_recovers_from_invalid_json(tmp_path):
    path = tmp_path / "settings.json"
    path.write_text("not-json", encoding="utf-8")
    assert SettingsStore(path).load() == UserSettings()

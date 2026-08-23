from stellar_sdk import Keypair

from fresnica.market_preferences import MarketPreferencesStore
from fresnica.models import Asset, MarketPair


def test_market_preferences_are_scoped_deduplicated_and_persistent(tmp_path):
    store = MarketPreferencesStore(tmp_path / "markets.json")
    address_a = Keypair.random().public_key
    address_b = Keypair.random().public_key
    issuer = Keypair.random().public_key
    pair_a = MarketPair(Asset("USD", issuer), Asset("XLM"))
    pair_b = MarketPair(Asset("EUR", issuer), Asset("XLM"))

    store.touch("mainnet", address_a, pair_a)
    store.touch("mainnet", address_a, pair_b)
    store.touch("mainnet", address_a, pair_a)
    preferences = store.toggle_favorite("mainnet", address_a, pair_a)

    assert preferences.recents == (pair_a, pair_b)
    assert preferences.favorites == (pair_a,)
    assert store.get("mainnet", address_b).recents == ()
    assert store.get("testnet", address_a).recents == ()

    reloaded = MarketPreferencesStore(tmp_path / "markets.json").get("mainnet", address_a)
    assert reloaded == preferences

    unstarred = store.toggle_favorite("mainnet", address_a, pair_a)
    assert unstarred.favorites == ()

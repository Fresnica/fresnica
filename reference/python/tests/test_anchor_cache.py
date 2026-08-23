from stellar_sdk import Keypair

from fresnica.anchor_cache import AnchorCapabilitiesStore
from fresnica.anchor_service import AnchorCapabilities
from fresnica.models import Asset


def test_anchor_capability_cache_round_trips_and_scopes_by_exact_asset(tmp_path):
    issuer = Keypair.random().public_key
    other_issuer = Keypair.random().public_key
    asset = Asset("USD", issuer)
    capabilities = AnchorCapabilities(
        domain="anchor.example",
        sep6_url="https://anchor.example/sep6",
        sep24_url="https://anchor.example/sep24",
        web_auth_url="https://anchor.example/auth",
        signing_key=Keypair.random().public_key,
        sep6_deposit=True,
        sep6_deposit_info={"enabled": True, "fee_fixed": "0"},
        sep6_withdraw_info={"enabled": True, "types": {"crypto": {"fields": {"dest": {}}}}},
        sep24_deposit=True,
        sep24_withdraw=True,
        warnings=("cached warning",),
    )

    path = tmp_path / "anchors.json"
    store = AnchorCapabilitiesStore(path)
    store.put(asset, capabilities)

    assert AnchorCapabilitiesStore(path).get(asset, "ANCHOR.EXAMPLE.") == capabilities
    assert store.get(Asset("USD", other_issuer), "anchor.example") is None
    assert store.get(asset, "other.example") is None

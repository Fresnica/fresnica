import json

from stellar_sdk import Keypair

from fresnica.anchor_service import AnchorService
from fresnica.models import Asset


class Response:
    def __init__(self, body, *, json_body=None):
        self.content = body.encode("utf-8")
        self._json = json_body

    def raise_for_status(self):
        return None

    def json(self):
        if self._json is not None:
            return self._json
        return json.loads(self.content)


class Session:
    def __init__(self, responses):
        self.responses = list(responses)
        self.urls = []

    def get(self, url, timeout):
        self.urls.append((url, timeout))
        return self.responses.pop(0)


def test_anchor_discovery_checks_exact_asset_and_sep6_sep24_info():
    issuer = Keypair.random().public_key
    toml = f'''TRANSFER_SERVER = "https://anchor.example/sep6"
TRANSFER_SERVER_SEP0024 = "https://anchor.example/sep24"
WEB_AUTH_ENDPOINT = "https://anchor.example/auth"
[[CURRENCIES]]
code = "USD"
issuer = "{issuer}"
'''
    session = Session(
        [
            Response(toml),
            Response("{}", json_body={"deposit": {"USD": {"enabled": True}}, "withdraw": {"USD": {"enabled": False}}}),
            Response("{}", json_body={"deposit": {"USD": {"enabled": True}}, "withdraw": {"USD": {"enabled": True}}}),
        ]
    )

    capabilities = AnchorService(session=session).discover(
        Asset("USD", issuer),
        "anchor.example",
    )

    assert capabilities.sep6_url == "https://anchor.example/sep6"
    assert capabilities.sep24_url == "https://anchor.example/sep24"
    assert capabilities.sep6_deposit is True
    assert capabilities.sep6_withdraw is False
    assert capabilities.sep24_deposit is True
    assert capabilities.sep24_withdraw is True
    assert capabilities.web_auth_url == "https://anchor.example/auth"
    assert session.urls[0][0] == "https://anchor.example/.well-known/stellar.toml"


def test_anchor_discovery_does_not_attribute_other_currency_to_asset():
    issuer = Keypair.random().public_key
    other = Keypair.random().public_key
    toml = f'''TRANSFER_SERVER = "https://anchor.example/sep6"
[[CURRENCIES]]
code = "USD"
issuer = "{other}"
'''
    session = Session([Response(toml)])

    capabilities = AnchorService(session=session).discover(
        Asset("USD", issuer),
        "anchor.example",
    )

    assert capabilities.sep6_url is None
    assert capabilities.warnings == ("stellar.toml does not list this exact asset",)
    assert len(session.urls) == 1

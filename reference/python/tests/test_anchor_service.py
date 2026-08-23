import json

import pytest
from stellar_sdk import Keypair, TransactionEnvelope
from stellar_sdk.sep.stellar_web_authentication import build_challenge_transaction

from fresnica.anchor_service import AnchorCapabilities, AnchorError, AnchorService
from fresnica.models import Asset
from fresnica.network import get_network
from fresnica.wallet import Wallet


class Response:
    def __init__(self, body="{}", *, json_body=None):
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
    signing = Keypair.random().public_key
    toml = f'''TRANSFER_SERVER = "https://anchor.example/sep6"
TRANSFER_SERVER_SEP0024 = "https://anchor.example/sep24"
WEB_AUTH_ENDPOINT = "https://anchor.example/auth"
SIGNING_KEY = "{signing}"
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
    assert capabilities.signing_key == signing
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


class Sep24Session:
    def __init__(self, challenge_xdr, client_public_key, network_passphrase):
        self.challenge_xdr = challenge_xdr
        self.client = Keypair.from_public_key(client_public_key)
        self.network_passphrase = network_passphrase
        self.calls = []

    def get(self, url, params=None, timeout=None):
        self.calls.append(("GET", url, params))
        return Response(json_body={"transaction": self.challenge_xdr})

    def post(self, url, data=None, json=None, headers=None, timeout=None):
        self.calls.append(("POST", url, data, json, headers))
        if url.endswith("/auth"):
            signed = TransactionEnvelope.from_xdr(
                json["transaction"],
                network_passphrase=self.network_passphrase,
            )
            assert len(signed.signatures) == 2
            self.client.verify(signed.hash(), signed.signatures[-1].signature)
            return Response(json_body={"token": "verified-jwt"})
        assert headers == {"Authorization": "Bearer verified-jwt"}
        assert data["asset_code"] == "USD"
        assert data["account"] == self.client.public_key
        return Response(
            json_body={
                "type": "interactive_customer_info_needed",
                "url": "https://anchor.example/interactive/tx-1",
                "id": "tx-1",
            }
        )


def test_sep24_authenticates_with_verified_challenge_and_wallet_signature():
    network_passphrase = get_network("testnet").passphrase
    server = Keypair.random()
    client = Keypair.random()
    issuer = Keypair.random().public_key
    challenge = build_challenge_transaction(
        server.secret,
        client.public_key,
        "anchor.example",
        "anchor.example",
        network_passphrase,
    )
    session = Sep24Session(challenge, client.public_key, network_passphrase)
    capabilities = AnchorCapabilities(
        domain="anchor.example",
        sep24_url="https://anchor.example/sep24",
        web_auth_url="https://anchor.example/auth",
        signing_key=server.public_key,
        sep24_deposit=True,
    )

    transfer = AnchorService(session=session).start_sep24(
        Wallet.from_secret(client.secret),
        Asset("USD", issuer),
        capabilities,
        "deposit",
        network_passphrase,
    )

    assert transfer.kind == "deposit"
    assert transfer.url == "https://anchor.example/interactive/tx-1"
    assert transfer.transaction_id == "tx-1"
    assert session.calls[0] == (
        "GET",
        "https://anchor.example/auth",
        {"account": client.public_key},
    )
    assert session.calls[-1][1] == "https://anchor.example/sep24/transactions/deposit/interactive"


def test_sep24_rejects_challenge_for_another_account():
    network_passphrase = get_network("testnet").passphrase
    server = Keypair.random()
    client = Keypair.random()
    other = Keypair.random()
    issuer = Keypair.random().public_key
    challenge = build_challenge_transaction(
        server.secret,
        other.public_key,
        "anchor.example",
        "anchor.example",
        network_passphrase,
    )
    session = Sep24Session(challenge, client.public_key, network_passphrase)
    capabilities = AnchorCapabilities(
        domain="anchor.example",
        sep24_url="https://anchor.example/sep24",
        web_auth_url="https://anchor.example/auth",
        signing_key=server.public_key,
        sep24_deposit=True,
    )

    with pytest.raises(AnchorError, match="different account"):
        AnchorService(session=session).start_sep24(
            Wallet.from_secret(client.secret),
            Asset("USD", issuer),
            capabilities,
            "deposit",
            network_passphrase,
        )

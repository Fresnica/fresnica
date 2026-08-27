import json

import pytest
from stellar_sdk import Keypair, TransactionEnvelope
from stellar_sdk.sep.stellar_web_authentication import build_challenge_transaction

from fresnica.anchor_service import AnchorCapabilities, AnchorError, AnchorService
from fresnica.errors import NetworkError
from fresnica.models import Asset
from fresnica.network import get_network
from fresnica.wallet import Wallet


class Response:
    def __init__(self, body="{}", *, json_body=None, status_code=200):
        self.content = body.encode("utf-8")
        self._json = json_body
        self.status_code = status_code

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

    def get(self, url, timeout, allow_redirects=True):
        assert allow_redirects is False
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


def test_anchor_discovery_requires_exact_case_and_issuer_identity():
    issuer = Keypair.random().public_key
    for currency in (
        f'''[[CURRENCIES]]
code = "usd"
issuer = "{issuer}"
''',
        '''[[CURRENCIES]]
code = "USD"
''',
    ):
        toml = f'''TRANSFER_SERVER = "https://anchor.example/sep6"
{currency}'''
        session = Session([Response(toml)])
        capabilities = AnchorService(session=session).discover(
            Asset("USD", issuer),
            "anchor.example",
        )
        assert capabilities.sep6_url is None
        assert capabilities.warnings == ("stellar.toml does not list this exact asset",)
        assert len(session.urls) == 1


def test_anchor_info_keys_are_case_sensitive():
    issuer = Keypair.random().public_key
    toml = f'''TRANSFER_SERVER = "https://anchor.example/sep6"
[[CURRENCIES]]
code = "USD"
issuer = "{issuer}"
'''
    session = Session(
        [
            Response(toml),
            Response(
                "{}",
                json_body={
                    "deposit": {"usd": {"enabled": True}},
                    "withdraw": {"usd": {"enabled": True}},
                },
            ),
        ]
    )
    capabilities = AnchorService(session=session).discover(
        Asset("USD", issuer),
        "anchor.example",
    )
    assert capabilities.sep6_deposit is False
    assert capabilities.sep6_withdraw is False


def test_anchor_discovery_rejects_http_redirects():
    session = Session([Response(status_code=302)])
    with pytest.raises(NetworkError, match="redirects are not allowed"):
        AnchorService(session=session).discover(
            Asset("USD", Keypair.random().public_key),
            "anchor.example",
        )


class Sep24Session:
    def __init__(self, challenge_xdr, client_public_key, network_passphrase):
        self.challenge_xdr = challenge_xdr
        self.client = Keypair.from_public_key(client_public_key)
        self.network_passphrase = network_passphrase
        self.calls = []

    def get(self, url, params=None, timeout=None, allow_redirects=True):
        assert allow_redirects is False
        self.calls.append(("GET", url, params))
        return Response(json_body={"transaction": self.challenge_xdr})

    def post(
        self,
        url,
        data=None,
        json=None,
        headers=None,
        timeout=None,
        allow_redirects=True,
    ):
        assert allow_redirects is False
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


class Sep6Session:
    def __init__(self, payload):
        self.payload = payload
        self.calls = []

    def get(
        self,
        url,
        params=None,
        headers=None,
        timeout=None,
        allow_redirects=True,
    ):
        assert allow_redirects is False
        self.calls.append((url, params, headers, timeout))
        return Response(json_body=self.payload)


def test_sep6_accepts_fchain_style_deposit_instructions_without_structured_address():
    issuer = Keypair.random().public_key
    wallet = Wallet.from_address(Keypair.random().public_key)
    session = Sep6Session(
        {
            "how": "Address: rDepositAddress, DT: 100005077",
            "extra_info": {"message": "You MUST include the DestinationTag."},
        }
    )
    capabilities = AnchorCapabilities(
        domain="fchain.io",
        sep6_url="https://api.fchain.io",
        sep6_deposit=True,
        sep6_deposit_info={"enabled": True, "fee_fixed": "0", "fee_percent": "0"},
    )

    transfer = AnchorService(session=session).start_sep6(
        wallet,
        Asset("XRP", issuer),
        capabilities,
        "deposit",
        get_network("mainnet").passphrase,
    )

    assert transfer.payload["how"].startswith("Address:")
    assert transfer.request == {"asset_code": "XRP", "account": wallet.address()}
    assert session.calls[0][0] == "https://api.fchain.io/deposit"


def test_sep6_withdraw_preserves_anchor_hash_memo_and_request_fields():
    issuer = Keypair.random().public_key
    wallet = Wallet.from_secret(Keypair.random().secret)
    memo = "AK4SOoVW88+RFUcRN2r7D4lPgys9xn9KUAAAAAAAAAA="
    session = Sep6Session(
        {
            "account_id": Keypair.random().public_key,
            "memo_type": "hash",
            "memo": memo,
            "fee_fixed": 0.001,
            "fee_percent": 0.1,
        }
    )
    capabilities = AnchorCapabilities(
        domain="fchain.io",
        sep6_url="https://api.fchain.io",
        sep6_withdraw=True,
        sep6_withdraw_info={
            "enabled": True,
            "types": {"crypto": {"fields": {"amount": {}, "dest": {}, "dest_extra": {"optional": True}}}},
        },
    )

    transfer = AnchorService(session=session).start_sep6(
        wallet,
        Asset("XRP", issuer),
        capabilities,
        "withdraw",
        get_network("mainnet").passphrase,
        {"amount": "5", "dest": "rExample", "dest_extra": "123"},
    )

    assert transfer.request["type"] == "crypto"
    assert transfer.request["amount"] == "5"
    assert transfer.payload["memo_type"] == "hash"
    assert transfer.payload["memo"] == memo

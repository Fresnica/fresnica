from stellar_sdk import Keypair

from fresnica.anchor_service import (
    AnchorCapabilities,
    AnchorInteractiveTransfer,
    AnchorSep6Transfer,
)
from fresnica.anchor_transfer_service import (
    AnchorDepositInstructions,
    AnchorKycRequired,
    AnchorOpenUrl,
    AnchorTransferService,
    AnchorWithdrawalPayment,
)
from fresnica.models import Asset
from fresnica.wallet import Wallet


class Protocol:
    def __init__(self, sep6_payload=None):
        self.sep6_payload = sep6_payload or {}
        self.calls = []

    def discover(self, asset, domain):
        self.calls.append(("discover", asset, domain))
        return AnchorCapabilities(domain=domain)

    def start_sep24(self, wallet, asset, capabilities, kind, network_passphrase):
        self.calls.append(("sep24", kind))
        return AnchorInteractiveTransfer(kind, "https://anchor.example/session", "tx-1")

    def start_sep6(self, wallet, asset, capabilities, kind, network_passphrase, fields):
        self.calls.append(("sep6", kind, dict(fields)))
        return AnchorSep6Transfer(
            kind=kind,
            payload=dict(self.sep6_payload),
            request={"asset_code": asset.code, "account": wallet.address(), **fields},
        )


def _asset():
    return Asset("XRP", Keypair.random().public_key)


def test_plan_prefers_usable_sep24_over_sep6():
    protocol = Protocol()
    service = AnchorTransferService(protocol)
    capabilities = AnchorCapabilities(
        domain="anchor.example",
        sep24_url="https://anchor.example/sep24",
        web_auth_url="https://anchor.example/auth",
        signing_key=Keypair.random().public_key,
        sep24_deposit=True,
        sep6_url="https://anchor.example/sep6",
        sep6_deposit=True,
    )
    wallet = Wallet.from_secret(Keypair.random().secret)

    plan = service.plan(capabilities, "deposit")
    outcome = service.start(
        wallet, _asset(), capabilities, "deposit", "network", plan=plan, fields={}
    )

    assert plan.protocol == "sep24"
    assert plan.requires_signing
    assert isinstance(outcome, AnchorOpenUrl)
    assert protocol.calls == [("sep24", "deposit")]


def test_sep6_plan_owns_fields_type_and_fchain_style_withdraw_interpretation():
    memo = "AK4SOoVW88+RFUcRN2r7D4lPgys9xn9KUAAAAAAAAAA="
    destination = Keypair.random().public_key
    protocol = Protocol(
        {
            "account_id": destination,
            "memo_type": "hash",
            "memo": memo,
            "extra_info": {"message": "Send exactly once"},
        }
    )
    service = AnchorTransferService(protocol)
    capabilities = AnchorCapabilities(
        domain="fchain.io",
        sep6_url="https://api.fchain.io",
        sep6_withdraw=True,
        sep6_withdraw_info={
            "enabled": True,
            "types": {
                "crypto": {
                    "fields": {
                        "amount": {},
                        "dest": {},
                        "dest_extra": {"optional": True},
                    }
                }
            },
        },
    )
    wallet = Wallet.from_secret(Keypair.random().secret)
    asset = _asset()

    plan = service.plan(capabilities, "withdraw")
    outcome = service.start(
        wallet,
        asset,
        capabilities,
        "withdraw",
        "network",
        plan=plan,
        fields={"amount": "5", "dest": "rExample"},
    )

    assert plan.protocol == "sep6"
    assert plan.transfer_type == "crypto"
    assert plan.requires_fields
    assert plan.requires_signing
    assert protocol.calls[-1][2]["type"] == "crypto"
    assert isinstance(outcome, AnchorWithdrawalPayment)
    assert outcome.asset == asset
    assert outcome.amount == "5"
    assert outcome.destination == destination
    assert outcome.memo_type == "hash"
    assert outcome.memo == memo
    assert outcome.extra_info == "Send exactly once"


def test_sep6_deposit_and_kyc_are_explicit_next_actions():
    wallet = Wallet.from_address(Keypair.random().public_key)
    asset = _asset()
    capabilities = AnchorCapabilities(
        domain="fchain.io",
        sep6_url="https://api.fchain.io",
        sep6_deposit=True,
        sep6_deposit_info={"enabled": True},
    )

    deposit_service = AnchorTransferService(
        Protocol({"how": "Address: rDeposit, DT: 42"})
    )
    deposit = deposit_service.start(
        wallet, asset, capabilities, "deposit", "network", fields={}
    )
    assert isinstance(deposit, AnchorDepositInstructions)
    assert deposit.payload["how"].startswith("Address:")

    kyc_service = AnchorTransferService(
        Protocol(
            {
                "type": "non_interactive_customer_info_needed",
                "fields": ["given_name"],
            }
        )
    )
    kyc = kyc_service.start(
        wallet, asset, capabilities, "deposit", "network", fields={}
    )
    assert isinstance(kyc, AnchorKycRequired)
    assert kyc.payload["fields"] == ["given_name"]

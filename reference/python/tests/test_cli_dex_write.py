from types import SimpleNamespace
from decimal import Decimal

from stellar_sdk import Keypair

from fresnica.cli.commands.dex import execute_dex
from fresnica.cli.parser import parse_args
from fresnica.models import Asset, MarketPair, OpenOffer, PriceRatio


class FakeManager:
    def __init__(self, wallet):
        self.record = SimpleNamespace(name="main", network="testnet", watch_only=False)
        self.session = SimpleNamespace(record=self.record, wallet=wallet)
        self.locked = False

    def get_record(self, name=None):
        return self.record

    def current(self):
        return self.session

    def lock(self):
        self.locked = True


class FakeDexService:
    def __init__(self, offers=None):
        self.offers = list(offers or [])

    def get_open_offers(self, wallet, limit=200, refresh=True):
        return self.offers


class FakeOfferService:
    def __init__(self):
        self.calls = []

    def prepare_create(self, wallet_name, wallet, intent, allow_trustline=False):
        self.calls.append(("create", intent, allow_trustline))
        return SimpleNamespace(review=SimpleNamespace(action="create"), envelope=object())

    def prepare_update(self, wallet_name, wallet, offer, intent):
        self.calls.append(("update", offer, intent))
        return SimpleNamespace(review=SimpleNamespace(action="update"), envelope=object())

    def prepare_cancel(self, wallet_name, wallet, offer):
        self.calls.append(("cancel", offer))
        return SimpleNamespace(review=SimpleNamespace(action="cancel"), envelope=object())

    def sign(self, wallet, prepared):
        self.calls.append(("sign",))

    def submit(self, prepared):
        self.calls.append(("submit",))
        return SimpleNamespace(hash="abc", ledger=5)


class FakeRenderer:
    def __init__(self):
        self.review = None
        self.result = None

    def render_offer_review(self, review):
        self.review = review

    def confirm(self):
        return True

    def render_result(self, result, network):
        self.result = result


def _runtime(dex_service, offer_service):
    wallet = SimpleNamespace(address=lambda: Keypair.random().public_key)
    manager = FakeManager(wallet)
    services = SimpleNamespace(dex_service=dex_service, offer_service=offer_service)
    return SimpleNamespace(
        network="testnet",
        wallet_manager=manager,
        services_for=lambda: services,
    )


def test_buy_command_builds_pair_relative_offer_intent():
    issuer = Keypair.random().public_key
    args = parse_args(
        [
            "--network",
            "testnet",
            "dex",
            "buy",
            f"XRP:{issuer}",
            "XLM",
            "100",
            "0.325",
            "--allow-trustline",
            "-y",
        ]
    )
    offers = FakeOfferService()
    runtime = _runtime(FakeDexService(), offers)
    renderer = FakeRenderer()

    execute_dex(runtime, args, renderer)
    kind, intent, allow_trustline = offers.calls[0]
    assert kind == "create"
    assert allow_trustline is True
    assert intent.side == "buy"
    assert intent.pair.base == Asset("XRP", issuer)
    assert intent.pair.counter == Asset("XLM")
    assert intent.amount == Decimal("100")
    assert intent.price == Decimal("0.325")
    assert offers.calls[-2:] == [("sign",), ("submit",)]
    assert runtime.wallet_manager.locked


def test_update_infers_side_from_current_offer_projection():
    issuer = Keypair.random().public_key
    pair = MarketPair(Asset("XRP", issuer), Asset("XLM"))
    offer = OpenOffer(
        offer_id="42",
        selling=pair.counter,
        buying=pair.base,
        selling_amount=Decimal("32.5"),
        price_r=PriceRatio(40, 13),
    )
    args = parse_args(
        [
            "--network",
            "testnet",
            "dex",
            "update",
            "42",
            f"XRP:{issuer}",
            "XLM",
            "90",
            "0.33",
            "-y",
        ]
    )
    offers = FakeOfferService()
    runtime = _runtime(FakeDexService([offer]), offers)

    execute_dex(runtime, args, FakeRenderer())
    kind, updated_offer, intent = offers.calls[0]
    assert kind == "update"
    assert updated_offer is offer
    assert intent.side == "buy"
    assert intent.amount == Decimal("90")
    assert intent.price == Decimal("0.33")


def test_cancel_does_not_require_original_operation_type():
    pair = MarketPair(Asset("XLM"), Asset("USD", Keypair.random().public_key))
    offer = OpenOffer(
        offer_id="7",
        selling=pair.base,
        buying=pair.counter,
        selling_amount=Decimal("10"),
        price_r=PriceRatio(2, 1),
    )
    args = parse_args(["--network", "testnet", "dex", "cancel", "7", "-y"])
    offers = FakeOfferService()
    runtime = _runtime(FakeDexService([offer]), offers)

    execute_dex(runtime, args, FakeRenderer())
    assert offers.calls[0] == ("cancel", offer)

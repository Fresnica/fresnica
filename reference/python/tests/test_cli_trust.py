from types import SimpleNamespace

from stellar_sdk import Keypair

from fresnica.cli.commands.trust import execute_trust
from fresnica.cli.parser import parse_args


class Manager:
    def __init__(self, record, wallet):
        self.record = record
        self.session = SimpleNamespace(record=record, wallet=wallet)
        self.locked = False

    def get_record(self, name=None):
        return self.record

    def current(self):
        return self.session

    def unlock(self, name, password):
        return self.session

    def lock(self):
        self.locked = True


class TrustlineService:
    def __init__(self):
        self.calls = []
        self.signed = False

    def prepare_add(self, wallet_name, wallet, asset, limit=None):
        self.calls.append(("add", wallet_name, asset, limit))
        return SimpleNamespace(review="add-review", envelope="tx")

    def prepare_limit(self, wallet_name, wallet, asset, limit):
        self.calls.append(("limit", wallet_name, asset, limit))
        return SimpleNamespace(review="limit-review", envelope="tx")

    def prepare_remove(self, wallet_name, wallet, asset):
        self.calls.append(("remove", wallet_name, asset))
        return SimpleNamespace(review="remove-review", envelope="tx")

    def sign(self, wallet, prepared):
        self.signed = True

    def submit(self, prepared):
        return SimpleNamespace(hash="trust-hash", ledger=12)


class Renderer:
    def __init__(self):
        self.review = None
        self.result = None

    def render_review(self, review):
        self.review = review

    def confirm(self):
        return True

    def render_result(self, result, network):
        self.result = (result, network.name)


class Runtime:
    def __init__(self):
        self.network = "testnet"
        self.wallet = SimpleNamespace(
            address=lambda: Keypair.random().public_key,
            can_sign=lambda: True,
        )
        self.record = SimpleNamespace(
            name="main",
            network="testnet",
            address=Keypair.random().public_key,
            watch_only=False,
        )
        self.wallet_manager = Manager(self.record, self.wallet)
        self.trustline_service = TrustlineService()
        self.services = SimpleNamespace(
            trustline_service=self.trustline_service,
            pending_transaction_service=None,
        )

    def services_for(self):
        return self.services


def test_trust_parser_exposes_explicit_add_limit_remove_shapes():
    issuer = Keypair.random().public_key
    asset = f"USD:{issuer}"

    add = parse_args(["--network", "testnet", "trust", "add", asset, "--limit", "100"])
    assert add.command == "trust"
    assert add.trust_command == "add"
    assert add.asset == asset
    assert add.limit == "100"

    limit = parse_args(["trust", "limit", asset, "250"])
    assert limit.trust_command == "limit"
    assert limit.limit == "250"

    remove = parse_args(["trust", "remove", asset, "-y"])
    assert remove.trust_command == "remove"
    assert remove.yes is True


def test_trust_add_runs_review_sign_submit_and_locks_cli_session():
    issuer = Keypair.random().public_key
    asset = f"USD:{issuer}"
    runtime = Runtime()
    renderer = Renderer()
    args = parse_args(
        ["--network", "testnet", "trust", "add", asset, "--limit", "100", "-y"]
    )

    result = execute_trust(runtime, args, renderer)

    assert runtime.trustline_service.calls == [("add", "main", asset, "100")]
    assert renderer.review == "add-review"
    assert runtime.trustline_service.signed is True
    assert result.hash == "trust-hash"
    assert renderer.result == (result, "testnet")
    assert runtime.wallet_manager.locked is True

from types import SimpleNamespace

import pytest

from fresnica.cli.commands.balance import execute_balance
from fresnica.cli.commands.fund import execute_fund
from fresnica.cli.commands.send import execute_send
from fresnica.cli.commands.wallet import execute_wallet
from fresnica.cli.parser import parse_args
from fresnica.errors import NetworkError


class Record:
    def __init__(
        self,
        name="wallet",
        network="testnet",
        address="GTEST",
        watch_only=False,
    ):
        self.name = name
        self.network = network
        self.address = address
        self.watch_only = watch_only


class Manager:
    def __init__(self, record=None):
        self.record = record or Record()
        self.storage = SimpleNamespace(get_default=lambda: self.record.name)
        self.created = None
        self.locked = False

    def get_record(self, name=None):
        return self.record

    def view(self, name=None):
        return SimpleNamespace(record=self.record, wallet="watch-wallet")

    def has_app_passcode(self):
        return False

    def import_mnemonic(self, name, mnemonic, password, **kwargs):
        self.created = kwargs
        return Record(name=name, network=kwargs["network"])

    def create_mnemonic(self, name, password, **kwargs):
        self.created = kwargs
        return Record(name=name, network=kwargs["network"]), "generated mnemonic"

    def current(self):
        return None

    def unlock(self, name, password):
        return SimpleNamespace(record=self.record, wallet="signing-wallet")

    def lock(self):
        self.locked = True


class Renderer:
    def __init__(self):
        self.messages = []
        self.results = []

    def success(self, message):
        self.messages.append(message)

    def render_created_mnemonic(self, record, mnemonic):
        self.created = (record, mnemonic)

    def render_balance(self, record, views):
        self.balance = (record, views)

    def render_balance_json(self, record, views):
        self.balance_json = (record, views)

    def render_review(self, review):
        self.review = review

    def render_result(self, result, network):
        self.results.append((result, network.name))

    def confirm(self):
        return True


class TransferService:
    def prepare(self, **kwargs):
        self.prepared_kwargs = kwargs
        return SimpleNamespace(review="review", envelope="transaction")

    def sign(self, wallet, prepared):
        self.signed = (wallet, prepared)

    def submit(self, prepared):
        return SimpleNamespace(hash="hash", ledger=7)


class Runtime:
    def __init__(self, network="testnet", record=None, contacts=None):
        self.network = network
        self.wallet_manager = Manager(record)
        self.datastore = SimpleNamespace(get_balances=lambda network, address: [])
        self.transfer_service = TransferService()
        contacts = contacts or {}
        self.contact_store = SimpleNamespace(find=lambda name: contacts.get(name.casefold()))
        self.services = SimpleNamespace(
            testnet_service=(
                SimpleNamespace(fund=lambda address: {"hash": "friendbot-hash"})
                if network == "testnet"
                else None
            ),
            balance_service=SimpleNamespace(get_views=lambda wallet: ["XLM"]),
            transfer_service=self.transfer_service,
        )

    def services_for(self):
        return self.services


def test_wallet_create_uses_runtime_network():
    runtime = Runtime("testnet")
    args = parse_args(["--network", "testnet", "wallet", "create", "demo"])
    renderer = Renderer()
    secrets = iter(["", "password", "password"])

    execute_wallet(
        runtime,
        args,
        renderer,
        secret_input=lambda prompt: next(secrets),
    )

    assert runtime.wallet_manager.created["network"] == "testnet"
    assert renderer.created[1] == "generated mnemonic"


def test_wallet_fund_uses_default_testnet_wallet():
    runtime = Runtime("testnet", Record(name="demo", address="GABC"))
    args = parse_args(["--network", "testnet", "wallet", "fund"])
    renderer = Renderer()

    result = execute_fund(runtime, args, renderer)

    assert result["hash"] == "friendbot-hash"
    assert 'Funded wallet "demo" on testnet' in renderer.messages[0]


def test_wallet_fund_rejects_mainnet():
    runtime = Runtime("mainnet", Record(network="mainnet"))
    args = parse_args(["--network", "mainnet", "wallet", "fund"])

    with pytest.raises(NetworkError, match="only available on testnet"):
        execute_fund(runtime, args, Renderer())


def test_balance_rejects_network_mismatch():
    runtime = Runtime("testnet", Record(network="mainnet"))
    args = parse_args(["--network", "testnet", "balance"])

    with pytest.raises(NetworkError, match="configured for mainnet"):
        execute_balance(runtime, args, Renderer())


def test_send_uses_runtime_network_and_locks_wallet():
    runtime = Runtime("testnet", Record(network="testnet"))
    args = parse_args(
        ["--network", "testnet", "send", "1", "XLM", "to", "GDEST", "-y"]
    )
    renderer = Renderer()

    result = execute_send(
        runtime,
        args,
        renderer,
        password_provider=lambda prompt: "password",
    )

    assert result.hash == "hash"
    assert renderer.results == [(result, "testnet")]
    assert runtime.wallet_manager.locked is True


def test_send_resolves_contact_and_uses_default_memo():
    contact = SimpleNamespace(name="Alice", address="GALICE", memo="account-42")
    runtime = Runtime("testnet", contacts={"alice": contact})
    args = parse_args(["--network", "testnet", "send", "1", "XLM", "to", "Alice", "-y"])

    execute_send(
        runtime,
        args,
        Renderer(),
        password_provider=lambda prompt: "password",
    )

    prepared = runtime.transfer_service.prepared_kwargs
    assert prepared["destination"] == "GALICE"
    assert prepared["memo"] == "account-42"
    assert prepared["contact_name"] == "Alice"


def test_explicit_send_memo_overrides_contact_default():
    contact = SimpleNamespace(name="Alice", address="GALICE", memo="default")
    runtime = Runtime("testnet", contacts={"alice": contact})
    args = parse_args(
        [
            "--network",
            "testnet",
            "send",
            "1",
            "XLM",
            "to",
            "Alice",
            "--memo",
            "explicit",
            "-y",
        ]
    )

    execute_send(
        runtime,
        args,
        Renderer(),
        password_provider=lambda prompt: "password",
    )

    assert runtime.transfer_service.prepared_kwargs["memo"] == "explicit"

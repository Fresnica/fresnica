from types import SimpleNamespace

import fresnica.cli.commands.tui as tui_command
import fresnica.cli.main as cli_main


def test_network_only_invocation_enters_tui_with_requested_context(monkeypatch):
    created = []
    launched = []

    class FakeRuntime:
        def __init__(self, network="mainnet"):
            self.network = network
            created.append(network)

    monkeypatch.setattr(cli_main, "Runtime", FakeRuntime)
    monkeypatch.setattr(tui_command, "run", lambda runtime: launched.append(runtime))

    assert cli_main.main(["--network", "testnet"]) == 0
    assert created == ["testnet"]
    assert launched[0].network == "testnet"


def test_default_invocation_uses_core_labelled_tui_launcher(monkeypatch):
    runtime = SimpleNamespace(core_client=object())
    launched = []
    monkeypatch.setattr(tui_command, "run", lambda value: launched.append(value))

    assert cli_main.main([], runtime=runtime) == 0
    assert launched == [runtime]
    assert tui_command.core_subtitle(runtime) == "Stellar Wallet · Rust Core"
    assert (
        tui_command.core_subtitle(SimpleNamespace(core_client=None))
        == "Stellar Wallet · Python Reference"
    )
